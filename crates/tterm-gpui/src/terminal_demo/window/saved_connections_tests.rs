//! Acceptance tests for the saved-SSH picker over live split sessions.

mod early_query;
mod first_trust;
mod server;
mod ssh_query;
#[cfg(unix)]
mod vim;

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{MouseButton, TestAppContext, VisualTestContext};
use serde_json::json;
use tterm_core::profiles::launch::{
    SavedSshLaunch, SavedSshProfileSummary, launch_from_saved_profile,
};
use tterm_core::profiles::storage::upsert_profile;
use tterm_core::profiles::types::SavedProfile;
use tterm_core::ssh::store::{KnownHostRecord, now_unix_ms, save_known_host_entry};
use tterm_core::workspace::tab::{ConnectionType, Tab, TabConnection, TabType};
use tterm_terminal::terminal::Terminal;

use super::saved_connections::{CANCEL_BUTTON_ID, CONFIRM_BUTTON_ID, ERROR_ID, OPEN_BUTTON_ID};
use super::split_session_tests::make_workspace_with;
use super::split_state::Edge;
use super::*;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use server::ShellServer;

const OPENING_DEADLINE: Duration = Duration::from_secs(20);
const FIRST_ROW: &str = "saved-connections-row-0";
const SECOND_ROW: &str = "saved-connections-row-1";

fn shell() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".into(), "/q".into()]);
    #[cfg(not(windows))]
    let (program, args) = ("/bin/sh", vec!["-i".into()]);
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
}

fn harness<'a>(cx: &'a mut TestAppContext) -> Harness<'a> {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (root, visual) = cx.add_window_view(|_, cx| {
        make_workspace_with(&runtime, events, shell(), AppConfig::default(), cx)
    });
    Harness {
        _runtime: runtime,
        root,
        visual,
    }
}

fn ensure_storage() {
    static STORAGE: OnceLock<Result<(), String>> = OnceLock::new();
    let initialized = STORAGE.get_or_init(|| {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("target")
            });
        let target = if target.is_absolute() {
            target
        } else {
            std::env::current_dir()
                .map_err(|error| format!("Failed to resolve the test data directory: {error}"))?
                .join(target)
        };
        let root = target.join(format!("gpui-saved-connections-{}", std::process::id()));
        let paths = tterm_core::paths::AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            local_data_dir: root.join("local-data"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            download_dir: None,
            temp_dir: root.join("temp"),
        };
        tterm_core::config::paths::init_config_dir(&paths)?;
        tterm_core::db::init()?;
        Ok(())
    });
    initialized
        .as_ref()
        .expect("the isolated saved-connection database should open");
}

fn seed_profile(id: &str, name: &str, port: u16) -> SavedProfile {
    ensure_storage();
    let profile: SavedProfile = serde_json::from_value(json!({
        "id": id,
        "name": name,
        "connection_type": "ssh",
        "host": "127.0.0.1",
        "port": port,
        "username": "picker-test",
        "auth_method": "password",
        "keepalive_interval_secs": 15,
        "keepalive_count_max": 3
    }))
    .expect("the seeded profile should deserialize");
    tterm_core::db::write(|transaction| upsert_profile(transaction, &profile))
        .expect("the seeded profile should be stored");
    profile
}

fn trust_server(id: &str, name: &str, server: &ShellServer) {
    ensure_storage();
    save_known_host_entry(KnownHostRecord {
        profile_id: Some(id.to_string()),
        profile_name: name.to_string(),
        host: "127.0.0.1".to_string(),
        port: server.port(),
        algorithm: server.algorithm(),
        fingerprint: server.fingerprint(),
        trusted_at: now_unix_ms(),
    })
    .expect("the loopback host should be trusted");
}

fn summary(id: &str, name: &str, port: u16) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some("picker-test".to_string()),
        host: Some("127.0.0.1".to_string()),
        port: Some(port),
        group_path: None,
    }
}

fn split_workspace(harness: &mut Harness<'_>) -> [Rc<TabBinding>; 2] {
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            assert!(
                workspace.splits.resize(1, &[], &[0.6, 0.4]),
                "the split ratio should be adjustable before the picker opens"
            );
            for id in [1, 2] {
                let binding = workspace
                    .tabs
                    .get(id)
                    .and_then(|tab| tab.binding.as_ref())
                    .expect("both split panes run a live PTY");
                assert!(binding.session().is_some());
            }
            [1, 2].map(|id| {
                Rc::clone(
                    workspace
                        .tabs
                        .get(id)
                        .and_then(|tab| tab.binding.as_ref())
                        .expect("the split binding"),
                )
            })
        })
    })
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

fn pump_until(visual: &mut VisualTestContext, what: &str, mut condition: impl FnMut(&App) -> bool) {
    let deadline = Instant::now() + OPENING_DEADLINE;
    loop {
        visual.run_until_parked();
        if visual.update(|_, cx| condition(cx)) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn picker<'a>(root: &'a Entity<Prototype>, cx: &'a App) -> &'a SavedConnectionsUi {
    root.read(cx).ui.saved_connections.read(cx)
}

fn open_picker(harness: &mut Harness<'_>) {
    click(harness.visual, OPEN_BUTTON_ID);
    let root = harness.root.clone();
    pump_until(harness.visual, "the picker to load", move |cx| {
        let picker = picker(&root, cx);
        picker.is_open() && (!picker.profiles().is_empty() || picker.error().is_some())
    });
}

fn active_view_is_focused(root: &Prototype, window: &Window, cx: &App) -> bool {
    root.tabs
        .active()
        .view
        .read(cx)
        .focus_handle()
        .is_focused(window)
}

fn output(model: &Terminal) -> String {
    model
        .snapshot()
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .map(|cell| cell.ch)
        .collect()
}

#[gpui_kit::test]
fn browsing_saved_connections_keeps_the_split_and_never_dials(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let split = split_workspace(&mut harness);
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.ui.saved_connections.update(cx, |picker, cx| {
                picker.set_profiles_for_test(
                    Ok(vec![
                        summary("alpha", "Alpha", server.port()),
                        summary("beta", "Beta", server.port()),
                    ]),
                    cx,
                );
            });
            assert!(active_view_is_focused(workspace, window, cx));
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        let picker = picker(&root, cx);
        assert!(picker.is_open());
        assert_eq!(picker.profiles().len(), 2);
        assert_eq!(picker.selected_profile_id(), Some("alpha"));
        assert!(!active_view_is_focused(workspace, window, cx));
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.tabs.active_id(), 2);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.6, 0.4]));
    });
    assert_eq!(server.connections(), 0, "browsing must not dial a host");
    harness.visual.simulate_keystrokes("down");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("beta"));
    });
    harness.visual.simulate_keystrokes("home");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("alpha"));
    });
    click(harness.visual, SECOND_ROW);
    harness.visual.update(|_window, cx| {
        let workspace = root.read(cx);
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("beta"));
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.6, 0.4]));
    });
    assert_eq!(server.connections(), 0, "selecting a row must not dial");
    harness.visual.simulate_keystrokes("escape");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert!(!picker(&root, cx).is_open());
        assert!(active_view_is_focused(workspace, window, cx));
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.tabs.active_id(), 2);
    });
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.open_saved_connections(window, cx);
        });
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert!(picker(&root, cx).is_open());
        assert_eq!(picker(&root, cx).selected_profile_id(), Some("alpha"));
        let before = workspace.tabs.len();
        root.update(cx, |workspace, cx| {
            workspace.open_saved_connections(window, cx);
            assert_eq!(workspace.tabs.len(), before);
        });
        assert_eq!(picker(&root, cx).profiles().len(), 2);
    });
    assert_eq!(server.connections(), 0);
    harness.visual.update(|window, cx| {
        let workspace = root.read(cx);
        for id in [1, 2] {
            let binding = workspace
                .tabs
                .get(id)
                .and_then(|tab| tab.binding.as_ref())
                .expect("the original split binding");
            assert!(Rc::ptr_eq(&split[id - 1], binding));
        }
        window.remove_window();
    });
}

#[gpui_kit::test]
fn a_failed_profile_read_shows_an_error_and_opens_nothing(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let root = harness.root.clone();
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(1, window, cx);
        });
        window.render_frame(cx);
    });
    harness.visual.update(|_window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.ui.saved_connections.update(cx, |picker, cx| {
                picker.set_profiles_for_test(Err("profile data is corrupt".to_string()), cx);
            });
        });
    });
    open_picker(&mut harness);
    harness.visual.update(|window, cx| window.render_frame(cx));
    let error_visible = harness.visual.debug_bounds(ERROR_ID).is_some();
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let picker = picker(&root, cx);
        assert!(picker.is_open());
        assert!(picker.error().is_some());
        assert!(picker.profiles().is_empty());
        assert!(picker.selected_profile_id().is_none());
        assert_eq!(workspace.tabs.len(), 1);
    });
    assert!(error_visible, "the load error should have a visible status");
    let no_rows = harness.visual.debug_bounds(FIRST_ROW).is_none();
    assert!(no_rows, "an unreadable source must not render profile rows");
    click(harness.visual, CONFIRM_BUTTON_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert!(!picker(&root, cx).is_opening());
        assert_eq!(workspace.tabs.len(), 1);
        assert_eq!(workspace.tabs.active_id(), 1);
    });
    assert_eq!(server.connections(), 0);
    click(harness.visual, CANCEL_BUTTON_ID);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(!picker(&root, cx).is_open());
        assert!(active_view_is_focused(root.read(cx), window, cx));
        window.remove_window();
    });
}

#[gpui_kit::test]
fn confirming_once_opens_one_tab_and_one_dial_under_repeat_input(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let name = "Picker Live";
    seed_profile("picker-live", name, server.port());
    trust_server("picker-live", name, &server);
    let split = split_workspace(&mut harness);
    let root = harness.root.clone();
    click(harness.visual, OPEN_BUTTON_ID);
    pump_until(harness.visual, "the saved profile list", {
        let root = root.clone();
        move |cx| {
            picker(&root, cx)
                .profiles()
                .iter()
                .any(|profile| profile.id == "picker-live")
        }
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        root.update(cx, |workspace, cx| {
            workspace
                .ui
                .saved_connections
                .update(cx, |picker, cx| picker.select_profile("picker-live", cx));
        });
    });
    let database = tterm_core::db::get()
        .expect("the isolated database is initialized")
        .lock()
        .expect("the isolated database lock");
    click(harness.visual, CONFIRM_BUTTON_ID);
    harness.visual.update(|_window, cx| {
        assert!(picker(&root, cx).is_open());
        assert!(picker(&root, cx).is_opening());
        assert_eq!(root.read(cx).tabs.len(), 2);
    });
    click(harness.visual, CONFIRM_BUTTON_ID);
    harness.visual.simulate_keystrokes("enter");
    harness.visual.update(|_window, cx| {
        assert!(picker(&root, cx).is_open());
        assert!(picker(&root, cx).is_opening());
        assert_eq!(root.read(cx).tabs.len(), 2);
    });
    assert_eq!(
        server.connections(),
        0,
        "no dial before the profile is read"
    );
    drop(database);
    pump_until(harness.visual, "the picker to finish opening", {
        let root = root.clone();
        move |cx| !picker(&root, cx).is_open()
    });
    pump_until(harness.visual, "the first dial", |_| {
        server.connections() == 1
    });
    std::thread::sleep(Duration::from_secs(1));
    harness.visual.run_until_parked();
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert_eq!(
            server.connections(),
            1,
            "only one connection should be dialed"
        );
        assert_eq!(workspace.tabs.len(), 3);
        assert_eq!(workspace.tabs.active_id(), 3);
        assert_eq!(workspace.visible_ids(), vec![3]);
        let opened = workspace.tabs.get(3).expect("the saved tab");
        assert_eq!(opened.saved_id(), "tab-3");
        assert_eq!(
            opened
                .saved()
                .saved_field("connection")
                .and_then(|connection| connection.get("profileId"))
                .and_then(|value| value.as_str()),
            Some("picker-live")
        );
        for id in [1, 2] {
            let binding = workspace
                .tabs
                .get(id)
                .and_then(|tab| tab.binding.as_ref())
                .expect("the original split binding");
            assert!(Rc::ptr_eq(&split[id - 1], binding));
        }
    });
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(2, window, cx);
        });
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.tabs.active_id(), 2);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.6, 0.4]));
        assert!(active_view_is_focused(workspace, window, cx));
    });
    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_saved_launch_authenticates_once_and_keeps_the_existing_split_alive(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let name = "Direct Live";
    trust_server("direct-live", name, &server);
    let split = split_workspace(&mut harness);
    let root = harness.root.clone();
    let launch = direct_launch("direct-live", name, &server);
    let new_id = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace
                .open_saved_profile_launch(launch, window, cx)
                .expect("the direct saved launch should open one tab")
        })
    });
    assert_eq!(new_id, 3);
    let model = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 3);
        assert_eq!(workspace.tabs.active_id(), new_id);
        assert_eq!(workspace.visible_ids(), vec![new_id]);
        let opened = workspace.tabs.get(new_id).expect("the saved tab");
        assert_eq!(opened.saved_id(), "tab-3");
        assert_eq!(
            opened
                .saved()
                .saved_field("connection")
                .and_then(|connection| connection.get("profileId"))
                .and_then(|value| value.as_str()),
            Some("direct-live")
        );
        workspace
            .tabs
            .get(new_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch runs a live session")
            .model()
    });
    pump_until(harness.visual, "the SSH shell to become ready", |_| {
        output(&model).contains("SAVED_SSH_READY")
    });
    assert_eq!(server.connections(), 1);
    assert_eq!(server.shells(), 1);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(active_view_is_focused(root.read(cx), window, cx));
    });
    harness
        .visual
        .simulate_keystrokes("p i c k e r space t y p e d enter");
    pump_until(
        harness.visual,
        "the SSH shell to receive typed input",
        |_| {
            server
                .inputs()
                .windows(b"picker typed".len())
                .any(|window| window == b"picker typed")
        },
    );
    assert_eq!(server.connections(), 1, "typing must not redial");
    harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(2, window, cx);
        });
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        assert_eq!(workspace.visible_ids(), vec![1, 2]);
        assert_eq!(workspace.tabs.active_id(), 2);
        assert_eq!(workspace.splits.split_sizes(1, &[]), Some(vec![0.6, 0.4]));
        assert!(active_view_is_focused(workspace, window, cx));
        for id in [1, 2] {
            let session = workspace
                .tabs
                .get(id)
                .and_then(|tab| tab.binding.as_ref())
                .and_then(|binding| binding.session())
                .expect("the surviving split session");
            let binding = workspace
                .tabs
                .get(id)
                .and_then(|tab| tab.binding.as_ref())
                .expect("the surviving split binding");
            assert!(Rc::ptr_eq(&split[id - 1], binding));
            assert!(Arc::ptr_eq(
                &split[id - 1].session().expect("the original session"),
                &session
            ));
        }
    });
    let survivor = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        root.read(cx)
            .tabs
            .get(2)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the surviving PTY")
            .model()
    });
    send_marker(harness.visual, &survivor, "survivorpane");
    harness.visual.update(|window, _| window.remove_window());
}

fn direct_launch(id: &str, name: &str, server: &ShellServer) -> SavedSshLaunch {
    let profile: SavedProfile = serde_json::from_value(json!({
        "id": id,
        "name": name,
        "connection_type": "ssh",
        "host": "127.0.0.1",
        "port": server.port(),
        "username": "picker-test",
        "auth_method": "password"
    }))
    .expect("the direct launch profile should deserialize");
    let mut launch = launch_from_saved_profile(&profile).expect("a launch from the saved profile");
    launch.connection.password = Some("disposable-test-password".to_string());
    launch.connection.ignore_saved_password = true;
    launch.connection.auth_method = Some("password".to_string());
    launch.connection.ssh_shell_integration = Some(false);
    launch.connection.session_log = Some("never".to_string());
    launch.saved_tab = saved_tab(id, name);
    launch
}

fn saved_tab(id: &str, name: &str) -> Tab {
    let mut tab = Tab::new("", name, TabType::Ssh);
    let mut connection = TabConnection::default();
    connection.set_connection_type(Some(ConnectionType::Ssh));
    connection.set_profile_id(Some(id.to_string()));
    connection.set_profile_name(Some(name.to_string()));
    tab.set_connection(Some(connection));
    tab
}

fn send_marker(visual: &mut VisualTestContext, model: &Terminal, marker: &str) {
    visual.update(|window, cx| window.render_frame(cx));
    let keys = format!(
        "e c h o space {} enter",
        marker
            .chars()
            .map(|letter| letter.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    visual.simulate_keystrokes(&keys);
    pump_until(visual, "the surviving PTY output", |_| {
        output(model).contains(marker)
    });
}
