use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{Entity, TestAppContext, VisualTestContext};
use tterm_core::session_restore::scrollback::{ScrollbackSnapshot, write_snapshots};
use tterm_core::session_restore::workspace::{WorkspaceSnapshot, encode_session};
use tterm_core::session_restore::write_session;
use tterm_terminal::color::{Color, NamedColor};
use tterm_terminal::terminal::{Scroll, Terminal};

use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::session::Session;

use super::restart::TestDatabase;
use super::{CommandSpec, Prototype, SaveQueue, StartupWorkspace, local_tab, make_startup_window};

/// The snapshot the Tauri version's own serializer wrote; see
/// `src/components/TerminalTab/legacySnapshotFixture.test.ts` for the generation
/// command and pinned package versions.
const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/gpui-migration/reports/assets/P2-19/legacy.json"
));

/// Set in the child process, which then uses the root the parent created.
const ROOT: &str = "TTERM_TEST_P219_TEMP_ROOT";

/// The saved tabs the seeded workspace opens, in order. Three have a usable
/// snapshot; the rest are damaged, truncated, oversized or absent.
const TABS: [&str; 7] = [
    "legacy-one",
    "legacy-two",
    "legacy-three",
    "legacy-broken",
    "legacy-truncated",
    "legacy-huge",
    "legacy-missing",
];

/// The tabs whose snapshots the reader must keep, sorted by id.
const RESTORED: [&str; 3] = ["legacy-one", "legacy-three", "legacy-two"];

/// A snapshot that would change the title, write the clipboard, notify, report
/// shell integration and answer queries if it went through the live parser.
/// The import must draw `P219-LINK` and `hostile-visible` and drop the rest.
const HOSTILE: &str = concat!(
    "\x1b]0;P219-TITLE\x07",
    "\x1b]52;c;P219CLIP\x07",
    "\x1b]52;c;?\x07",
    "\x1b]9;P219-NOTIFY\x07",
    "\x1b]133;A;P219-COMMAND\x07",
    "\x1b[c\x1b[6n\x1b[14t",
    "\x1b[?1049h\x1b[?1000h\x1b[?2004h\x1b[?25l\x1b[4h",
    "\x1b]8;;https://example.com\x07P219-LINK\x1b]8;;\x07\r\n",
    "hostile-visible\r\n",
);

fn fixture_content() -> String {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    fixture["serialized"].as_str().unwrap().to_owned()
}

/// Writes the Tauri-format snapshots and the saved workspace beside them.
fn seed(config: &Path) -> PathBuf {
    let dir = config.join("scrollback");
    fs::create_dir_all(&dir).unwrap();
    let fixture = fixture_content();
    write_snapshots(
        &dir,
        &[
            ScrollbackSnapshot {
                tab_id: "legacy-one".into(),
                content: fixture.clone(),
            },
            // A different line at the top proves the map is keyed by tab id
            // rather than by the order the tabs were opened.
            ScrollbackSnapshot {
                tab_id: "legacy-two".into(),
                content: format!("second-tab-only\r\n{fixture}"),
            },
            ScrollbackSnapshot {
                tab_id: "legacy-three".into(),
                content: HOSTILE.into(),
            },
        ],
    )
    .unwrap();
    fs::write(dir.join("legacy-broken.gz"), b"incomplete").unwrap();
    // The last gzip byte holds the trailer, so this snapshot cannot be read.
    let mut truncated = fs::read(dir.join("legacy-two.gz")).unwrap();
    truncated.truncate(truncated.len() - 1);
    fs::write(dir.join("legacy-truncated.gz"), truncated).unwrap();
    // The reader refuses a file this large before it reads it.
    let huge = fs::File::create(dir.join("legacy-huge.gz")).unwrap();
    huge.set_len(32 * 1024 * 1024 + 1).unwrap();
    drop(huge);
    // A tab that is no longer in the workspace must survive untouched.
    fs::write(dir.join("not-open.gz"), b"preserve this file").unwrap();
    let shell = if cfg!(windows) { "cmd" } else { "auto" };
    let snapshot = WorkspaceSnapshot {
        tabs: TABS
            .map(|id| {
                let mut tab = local_tab(serde_json::json!({
                    "type": "terminal",
                    "terminalShell": shell,
                }));
                tab.set_id(id.to_owned());
                tab
            })
            .into(),
        active_tab_id: Some("legacy-one".into()),
        groups: vec![],
    };
    tterm_core::db::write(|conn| write_session(conn, encode_session(snapshot, 1))).unwrap();
    dir
}

fn contents(dir: &Path) -> BTreeMap<OsString, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect()
}

fn hashes(files: &BTreeMap<OsString, Vec<u8>>) -> BTreeMap<OsString, u64> {
    files
        .iter()
        .map(|(name, bytes)| {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            (name.clone(), hasher.finish())
        })
        .collect()
}

fn retained(model: &Terminal) -> String {
    model
        .retained_rows(0, 128)
        .rows()
        .iter()
        .map(|row| String::from_utf16_lossy(row.units()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Core storage is process-global. Each scenario runs in a child process with
/// its own data root, so it neither races other tests for the singleton nor
/// opens a user's database. The parent returns `None` once the child passed.
fn isolated_root(test: &str) -> Option<PathBuf> {
    let Some(root) = std::env::var_os(ROOT) else {
        let temp = TestDatabase::new();
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(ROOT, &temp.directory)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            result.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return None;
    };
    let root = PathBuf::from(root).canonicalize().unwrap();
    assert_eq!(
        root.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("tterm-gpui-restart-")
    );
    Some(root)
}

/// Points the process-global config and database at `root` and returns the
/// config directory the Tauri client would have written its snapshots to.
fn init_root(runtime: &GpuiRuntime, root: &Path) -> PathBuf {
    let mut paths = runtime.services.shared.paths.as_ref().clone();
    paths.config_dir = root.join("config");
    fs::create_dir(&paths.config_dir).unwrap();
    tterm_core::config::paths::init_config_dir(&paths).unwrap();
    tterm_core::db::init().unwrap();
    paths.config_dir
}

fn sorted_ids(workspace: &StartupWorkspace) -> Vec<String> {
    let mut ids: Vec<_> = workspace.legacy.keys().cloned().collect();
    ids.sort();
    ids
}

fn open_startup<'a>(
    cx: &'a mut TestAppContext,
    runtime: &mut GpuiRuntime,
    workspace: StartupWorkspace,
    command: CommandSpec,
) -> (Entity<Prototype>, &'a mut VisualTestContext) {
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    let geometry = SaveQueue::start(runtime._runtime.handle(), Arc::new(|_| Ok(())));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    cx.add_window_view(|_, cx| {
        make_startup_window(runtime, events, workspace, geometry, command, cx)
    })
}

/// Every restored tab and its live session, in workspace order.
fn restored_sessions(
    view: &Entity<Prototype>,
    cx: &mut gpui_kit::App,
) -> Vec<(String, Arc<Session>)> {
    let workspace = view.read(cx);
    assert_eq!(workspace.tabs.len(), TABS.len());
    workspace
        .tabs
        .ordered()
        .map(|(_, tab)| {
            let session = tab
                .binding
                .as_ref()
                .and_then(|binding| binding.session())
                .unwrap_or_else(|| panic!("{} has no session", tab.saved_id()));
            assert!(session.is_live(), "{} did not start", tab.saved_id());
            (tab.saved_id().to_owned(), session)
        })
        .collect()
}

fn session_of<'a>(sessions: &'a [(String, Arc<Session>)], id: &str) -> &'a Session {
    &sessions
        .iter()
        .find(|(saved, _)| saved == id)
        .unwrap_or_else(|| panic!("{id} was not restored"))
        .1
}

/// Checks each tab shows its own snapshot, with its history, colors and
/// banner, and that the hostile snapshot leaves no visible side effect.
fn assert_restored(sessions: &[(String, Arc<Session>)]) {
    for (id, session) in sessions {
        let text = retained(&session.model());
        match id.as_str() {
            "legacy-one" => {
                assert!(text.contains("scrolled-off-0"), "missing scrolled-off line");
                assert!(text.contains("red 中文"), "missing SGR text");
                assert!(text.contains("$ cargo test"), "missing command line");
                assert!(
                    text.contains("Restored"),
                    "missing restore banner; retained={text:?}"
                );
                assert!(!text.contains("second-tab-only"));
                assert!(!text.contains("hostile-visible"));
                let model = session.model();
                let red_line = text
                    .lines()
                    .position(|line| line.contains("red 中文"))
                    .unwrap();
                model.scroll_to_retained_line(red_line as i64);
                let snapshot = model.snapshot();
                model.scroll(Scroll::Bottom);
                assert!(
                    snapshot.rows().iter().any(|row| {
                        row.cells().windows(3).any(|cells| {
                            cells.iter().map(|cell| cell.ch).eq("red".chars())
                                && cells
                                    .iter()
                                    .all(|cell| cell.foreground == Color::Named(NamedColor::Red))
                        })
                    }),
                    "red lost its palette color"
                );
            }
            "legacy-two" => {
                assert!(text.contains("second-tab-only"), "missing its own line");
                assert!(text.contains("scrolled-off-0"), "missing shared history");
                assert!(text.contains("Restored"), "missing restore banner");
                assert!(!text.contains("hostile-visible"));
            }
            "legacy-three" => {
                assert!(text.contains("hostile-visible"), "missing visible text");
                assert!(text.contains("P219-LINK"), "missing literal text");
                assert!(text.contains("Restored"), "missing restore banner");
                assert!(text.find("hostile-visible").unwrap() < text.find("Restored").unwrap());
                for leaked in ["P219-TITLE", "P219CLIP", "P219-NOTIFY", "P219-COMMAND"] {
                    assert!(!text.contains(leaked), "{leaked} was drawn");
                }
                assert!(
                    session.clipboard_writes().take().is_none(),
                    "the snapshot wrote the clipboard"
                );
                assert_ne!(
                    session.title().lock().unwrap().as_deref(),
                    Some("P219-TITLE"),
                    "the snapshot changed the title"
                );
                let model = session.model();
                let link_line = text
                    .lines()
                    .position(|line| line.contains("P219-LINK"))
                    .unwrap();
                model.scroll_to_retained_line(link_line as i64);
                let snapshot = model.snapshot();
                model.scroll(Scroll::Bottom);
                assert!(
                    snapshot
                        .rows()
                        .iter()
                        .flat_map(|row| row.cells())
                        .all(|cell| cell.hyperlink.is_none()),
                    "the snapshot created a hyperlink"
                );
            }
            _ => {
                assert!(!text.contains("Restored"), "{id} showed a banner");
                assert!(!text.contains("scrolled-off-0"), "{id} showed history");
                assert!(!text.contains("hostile-visible"), "{id} showed history");
            }
        }
    }
}

/// Types into the new session and proves its output lands below the banner
/// without the history being replayed into the shell.
fn assert_new_output_follows(visual: &mut VisualTestContext, sessions: &[(String, Arc<Session>)]) {
    let session = session_of(sessions, "legacy-one");
    let (probe, marker) = if cfg!(windows) {
        ("echo P219-%OS%\r\n", "P219-Windows_NT")
    } else {
        ("printf 'P219-%s\\n' FRESH\r\n", "P219-FRESH")
    };
    session.write_input(probe.as_bytes()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let text = loop {
        visual.run_until_parked();
        let text = retained(&session.model());
        if text.contains(marker) {
            break text;
        }
        assert!(Instant::now() < deadline, "output: {text}");
        std::thread::sleep(Duration::from_millis(20));
    };
    let banner = text.find("Restored").unwrap();
    assert!(
        text.find("scrolled-off-0").unwrap() < banner,
        "history must come before the banner"
    );
    assert!(
        banner < text.find(marker).unwrap(),
        "new output must come last"
    );
    assert_eq!(
        text.matches("scrolled-off-0").count(),
        1,
        "the history was replayed into the PTY"
    );
}

fn assert_unchanged(dir: &Path, before: &BTreeMap<OsString, Vec<u8>>) {
    let after = contents(dir);
    assert_eq!(hashes(&after), hashes(before), "a snapshot changed");
    assert_eq!(after, *before, "a snapshot was added or removed");
    assert!(after.contains_key(OsStr::new("not-open.gz")));
    assert!(!after.contains_key(OsStr::new("legacy-missing.gz")));
}

#[gpui_kit::test]
fn legacy_snapshot_startup(cx: &mut TestAppContext) {
    const TEST: &str =
        "terminal_demo::window::session_workspace::tests::legacy::legacy_snapshot_startup";
    let Some(root) = isolated_root(TEST) else {
        return;
    };
    let mut runtime = test_runtime();
    let config = init_root(&runtime, &root);
    let dir = seed(&config);
    let before = contents(&dir);
    let handle = runtime._runtime.handle().clone();
    let shell = CommandSpec::from_args(&[]).unwrap();
    let workspace = super::super::startup(&handle, shell.uses_saved_workspace(), true);
    assert_eq!(sorted_ids(&workspace), RESTORED);
    let persistence = workspace.persistence.clone();
    let (view, visual) = open_startup(cx, &mut runtime, workspace, shell);
    let sessions = visual.update(|_, cx| restored_sessions(&view, cx));
    assert_restored(&sessions);
    assert_new_output_follows(visual, &sessions);
    handle.block_on(persistence.flush_latest()).unwrap();
    assert_unchanged(&dir, &before);
    visual.update(|window, _| window.remove_window());
}

/// An explicit `--exec` or `--ssh` run, the turned-off setting and a missing
/// folder all skip the snapshots; only an ordinary launch with the setting on
/// reads them, and nothing in the folder is pruned.
#[gpui_kit::test]
fn legacy_snapshot_bypasses_explicit_launch_and_disabled_setting(_cx: &mut TestAppContext) {
    const TEST: &str = "terminal_demo::window::session_workspace::tests::legacy::legacy_snapshot_bypasses_explicit_launch_and_disabled_setting";
    let Some(root) = isolated_root(TEST) else {
        return;
    };
    let runtime = test_runtime();
    let config = init_root(&runtime, &root);
    let dir = seed(&config);
    let before = contents(&dir);
    let handle = runtime._runtime.handle();

    let exec = CommandSpec::from_args(&["--exec".into(), "echo".into()]).unwrap();
    assert!(!exec.uses_saved_workspace());
    let explicit = super::super::startup(handle, exec.uses_saved_workspace(), true);
    assert!(!explicit.restore);
    assert!(explicit.legacy.is_empty());

    let ssh = CommandSpec::from_args(&["--ssh".into(), "prod".into()]).unwrap();
    assert!(!ssh.uses_saved_workspace());
    let remote = super::super::startup(handle, ssh.uses_saved_workspace(), true);
    assert!(!remote.restore);
    assert!(remote.legacy.is_empty());

    let disabled = super::super::startup(handle, true, false);
    assert!(disabled.restore);
    assert!(disabled.legacy.is_empty());

    let enabled = super::super::startup(handle, true, true);
    assert!(enabled.restore);
    assert_eq!(sorted_ids(&enabled), RESTORED);
    assert_eq!(contents(&dir), before);

    fs::remove_dir_all(&dir).unwrap();
    let missing = super::super::startup(handle, true, true);
    assert!(missing.restore);
    assert!(missing.legacy.is_empty());
}
