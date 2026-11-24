use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::TestAppContext;
use tterm_core::session_restore::scrollback::{ScrollbackSnapshot, write_snapshots};
use tterm_core::session_restore::workspace::{WorkspaceSnapshot, encode_session};
use tterm_core::session_restore::write_session;
use tterm_terminal::color::{Color, NamedColor};
use tterm_terminal::terminal::Terminal;

use super::restart::TestDatabase;
use super::{CommandSpec, SaveQueue, local_tab, make_startup_window, test_runtime};

fn seed(config: &Path) -> PathBuf {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/assets/P2-19/legacy.json"
    )))
    .unwrap();
    let dir = config.join("scrollback");
    fs::create_dir_all(&dir).unwrap();
    write_snapshots(
        &dir,
        &[ScrollbackSnapshot {
            tab_id: "legacy-color".into(),
            content: fixture["serialized"].as_str().unwrap().to_owned(),
        }],
    )
    .unwrap();
    fs::write(dir.join("legacy-broken.gz"), b"incomplete").unwrap();
    fs::write(dir.join("not-open.gz"), b"preserve this file").unwrap();
    let snapshot = WorkspaceSnapshot {
        tabs: ["legacy-broken", "legacy-color", "legacy-missing"]
            .map(|id| {
                let shell = if cfg!(windows) { "cmd" } else { "auto" };
                let mut tab =
                    local_tab(serde_json::json!({ "type": "terminal", "terminalShell": shell }));
                tab.set_id(id.to_owned());
                tab
            })
            .into(),
        active_tab_id: Some("legacy-color".into()),
        groups: vec![],
    };
    tterm_core::db::write(|conn| write_session(conn, encode_session(snapshot, 1))).unwrap();
    dir
}

fn contents(dir: &Path) -> std::collections::BTreeMap<std::ffi::OsString, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
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

/// Core storage is process-global. The child exercises ordinary startup without
/// racing other tests for the singleton or opening a user's database.
#[gpui_kit::test]
fn legacy_snapshot_startup(cx: &mut TestAppContext) {
    const ROOT: &str = "TTERM_TEST_P219_TEMP_ROOT";
    let Some(root) = std::env::var_os(ROOT) else {
        let temp = TestDatabase::new();
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal_demo::window::session_workspace::tests::legacy::legacy_snapshot_startup",
                "--nocapture",
            ])
            .env(ROOT, &temp.directory)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            result.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    let mut runtime = test_runtime();
    let mut paths = runtime.services.shared.paths.as_ref().clone();
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
    paths.config_dir = root.join("config");
    fs::create_dir(&paths.config_dir).unwrap();
    tterm_core::config::paths::init_config_dir(&paths).unwrap();
    tterm_core::db::init().unwrap();
    let dir = seed(&paths.config_dir);
    let before = contents(&dir);
    let handle = runtime._runtime.handle();
    let exec = CommandSpec::from_args(&["--exec".into(), "echo".into()]).unwrap();
    let explicit = super::super::startup(handle, exec.uses_saved_workspace(), true);
    assert!(!explicit.restore && explicit.legacy.is_empty());
    assert!(super::super::startup(handle, true, false).legacy.is_empty());
    let shell = CommandSpec::from_args(&[]).unwrap();
    let workspace = super::super::startup(handle, shell.uses_saved_workspace(), true);
    let persistence = workspace.persistence.clone();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    let geometry = SaveQueue::start(handle, Arc::new(|_| Ok(())));
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (view, visual) = cx.add_window_view(|_, cx| {
        make_startup_window(&runtime, events, workspace, geometry, shell, cx)
    });
    let model = visual.update(|_, cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        for (_, tab) in workspace.tabs.ordered() {
            let session = tab.binding.as_ref().unwrap().session().unwrap();
            assert!(session.is_live());
            let has_history = tab.saved_id() == "legacy-color";
            let text = retained(&session.model());
            assert_eq!(text.contains("scrolled-off-0"), has_history);
            assert_eq!(text.contains("Restored"), has_history);
        }
        let session = workspace
            .tabs
            .active()
            .binding
            .as_ref()
            .unwrap()
            .session()
            .unwrap();
        let model = session.model();
        assert!(
            model
                .snapshot()
                .rows()
                .iter()
                .any(|row| row.cells().windows(3).any(|cells| cells
                    .iter()
                    .map(|c| c.ch)
                    .eq("red".chars())
                    && cells
                        .iter()
                        .all(|c| c.foreground == Color::Named(NamedColor::Red))))
        );
        let probe = if cfg!(windows) {
            "echo P219-%OS%\r\n"
        } else {
            "printf 'P219-%s\\n' FRESH\n"
        };
        session.write_input(probe.as_bytes()).unwrap();
        model
    });
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        visual.run_until_parked();
        let text = retained(&model);
        let marker = if cfg!(windows) {
            "P219-Windows_NT"
        } else {
            "P219-FRESH"
        };
        if let Some(index) = text.find(marker) {
            assert!(text.find("Restored").unwrap() < index);
            break;
        }
        assert!(Instant::now() < deadline, "output: {text}");
        std::thread::sleep(Duration::from_millis(20));
    }
    handle.block_on(persistence.flush_latest()).unwrap();
    assert_eq!(contents(&dir), before);
    visual.update(|window, _| window.remove_window());
}
