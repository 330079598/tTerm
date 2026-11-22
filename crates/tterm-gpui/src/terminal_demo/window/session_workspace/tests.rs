use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::{AppContext, Context, TestAppContext, VisualTestContext};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::events::AppEvent;
use tterm_core::i18n::language::Language;
use tterm_core::session_restore::SessionData;
use tterm_core::session_restore::workspace::{WorkspaceLoad, WorkspaceSnapshot, decode_session};
use tterm_core::workspace::tab::{Tab, TabType};

use super::launch::launch_for_tab;
use super::save::{SAVE_DEBOUNCE, WorkspacePersistence, test_persistence};
use super::{StartupWorkspace, can_save, snapshot};
use crate::app_window::background;
use crate::app_window::logical_bounds::LogicalBounds;
use crate::app_window::state::{SaveQueue, SavedWindowState, WindowGeometry};
use crate::config_state::ConfigState;
use crate::locale::{Locale, LocaleGlobal};
use crate::terminal_demo::binding::SessionManager;
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::keymap::{KeymapRouter, encoder_config};
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::window::split_state::SplitState;
use crate::terminal_demo::window::{Prototype, PrototypeStartup};

mod restart;

const WORKSPACE_FIXTURE: &str =
    include_str!("../../../../../tterm-core/tests/fixtures/session/gpui-v1-workspace.json");

fn fixture() -> WorkspaceSnapshot {
    let session: SessionData = serde_json::from_str(WORKSPACE_FIXTURE).expect("workspace fixture");
    match decode_session(session) {
        WorkspaceLoad::Readable { snapshot, .. } => snapshot,
        other => panic!("expected readable workspace, got {other:?}"),
    }
}

fn saved_snapshot(id: &str) -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        tabs: vec![Tab::new(id, "", TabType::Terminal)],
        active_tab_id: Some(id.to_string()),
        groups: Vec::new(),
    }
}

fn test_workspace<'a>(
    cx: &'a mut TestAppContext,
    writes: Arc<Mutex<Vec<&'static str>>>,
    notified: mpsc::Sender<()>,
) -> (
    GpuiRuntime,
    gpui_kit::Entity<Prototype>,
    &'a mut VisualTestContext,
) {
    test_workspace_loading(cx, writes, notified, WorkspaceLoad::Empty, false)
}

fn test_workspace_loading<'a>(
    cx: &'a mut TestAppContext,
    writes: Arc<Mutex<Vec<&'static str>>>,
    notified: mpsc::Sender<()>,
    load: WorkspaceLoad,
    restore: bool,
) -> (
    GpuiRuntime,
    gpui_kit::Entity<Prototype>,
    &'a mut VisualTestContext,
) {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    let handle = runtime._runtime.handle();
    let workspace_writes = Arc::clone(&writes);
    let workspace_notified = notified.clone();
    let persistence = test_persistence(handle, true, move |_| {
        workspace_writes.lock().expect("writes").push("workspace");
        let _ = workspace_notified.send(());
        Box::pin(async { Ok(()) })
    });
    let geometry_writes = Arc::clone(&writes);
    let geometry_queue = SaveQueue::start(
        handle,
        Arc::new(move |_| {
            geometry_writes.lock().expect("writes").push("geometry");
            let _ = notified.send(());
            Ok(())
        }),
    );
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (root, visual) = cx.add_window_view(|_, cx| {
        make_workspace(
            &runtime,
            events,
            persistence,
            geometry_queue,
            load,
            restore,
            cx,
        )
    });
    (runtime, root, visual)
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    persistence: WorkspacePersistence,
    geometry_queue: SaveQueue,
    load: WorkspaceLoad,
    restore: bool,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let config = AppConfig::default();
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let geometry = cx.new(|_| {
        WindowGeometry::new(
            Some(SavedWindowState {
                bounds: LogicalBounds {
                    x: 40.0,
                    y: 40.0,
                    width: 1024.0,
                    height: 720.0,
                },
                maximized: false,
            }),
            Default::default(),
            geometry_queue,
        )
    });
    let config = cx.new(|_| ConfigState::new(config, handle));
    Prototype::new(
        cx,
        PrototypeStartup {
            command: CommandSpec::Shell,
            workspace: StartupWorkspace {
                load,
                persistence,
                restore,
            },
            sessions: Rc::new(SessionManager::new(
                runtime.services.clone(),
                Default::default(),
            )),
            events,
            keyboard: encoder_config(cfg!(target_os = "macos"), false),
            keymap,
            locale,
            config,
            theme: background::theme(None),
            background: background::content(None),
        },
        geometry,
    )
}

fn local_tab(connection: serde_json::Value) -> Tab {
    serde_json::from_value(serde_json::json!({
        "id": "local-1",
        "title": "Shell",
        "type": "terminal",
        "connection": connection,
    }))
    .expect("local tab")
}

fn saved_local_tab(id: &str) -> Tab {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "title": id,
        "type": "terminal",
        "connection": {
            "type": "terminal",
            "terminalShell": "auto",
        },
    }))
    .expect("saved local tab")
}

#[test]
fn persisted_ids_round_trip_through_numeric_pane_keys() {
    let snapshot = fixture();
    let ids = HashMap::from([("local shell 1", 4usize), ("ssh:prod host", 9usize)]);
    let splits = SplitState::restore(&snapshot.groups, |id| ids.get(id).copied());

    assert_eq!(splits.group_for(4), splits.group_for(9));
    assert_eq!(splits.zoomed(9), Some(9));
    assert_eq!(splits.visible_ids(9), vec![9]);
    assert_eq!(
        splits.saved_groups(|id| match id {
            4 => Some("local shell 1".to_string()),
            9 => Some("ssh:prod host".to_string()),
            _ => None,
        }),
        snapshot.groups
    );
}

#[gpui_kit::test]
fn restoring_enters_only_the_saved_active_tab(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let (notified, _notifications) = mpsc::channel();
    let load = WorkspaceLoad::Readable {
        snapshot: WorkspaceSnapshot {
            tabs: vec![saved_local_tab("tab-1"), saved_local_tab("tab-2")],
            active_tab_id: Some("tab-2".to_string()),
            groups: Vec::new(),
        },
        can_save: true,
    };
    let (_runtime, root, visual) = test_workspace_loading(cx, writes, notified, load, true);

    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            for (_, tab) in workspace.tabs.ordered() {
                assert!(
                    tab.binding
                        .as_ref()
                        .is_some_and(|binding| !binding.is_active()),
                    "opening a restored tab must not mark it visible"
                );
            }
            workspace.initialize_workspace(window, cx);
        });
    });

    visual.update(|_, cx| {
        root.update(cx, |workspace, cx| {
            let active = workspace
                .tabs
                .ordered()
                .map(|(id, tab)| {
                    (
                        id,
                        tab.binding
                            .as_ref()
                            .is_some_and(|binding| binding.is_active()),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(active, [(1, false), (2, true)]);
            let visible = workspace
                .tabs
                .ordered()
                .map(|(id, tab)| (id, tab.view.read(cx).is_visible()))
                .collect::<Vec<_>>();
            assert_eq!(visible, [(1, false), (2, true)]);
        });
    });
}

#[test]
fn tabs_missing_from_the_saved_layout_get_their_own_card() {
    let snapshot = fixture();
    let ids = HashMap::from([("local shell 1", 4usize), ("ssh:prod host", 9usize)]);
    let mut splits = SplitState::restore(&snapshot.groups, |id| ids.get(id).copied());

    assert!(splits.group_for(4).is_some());
    assert!(splits.group_for(9).is_some());
    splits.open(12);
    assert_eq!(splits.visible_ids(12), vec![12]);
    assert!(splits.group_for(12).is_some());
}

#[test]
fn an_unsupported_saved_tab_stays_inert_and_keeps_its_fields() {
    let tab: Tab = serde_json::from_value(serde_json::json!({
        "id": "serial-1",
        "title": "Serial",
        "type": "serial",
        "futureTabField": {"kept": true},
        "connection": {
            "type": "serial",
            "port": "COM3",
            "loginScript": "echo this must not run",
        },
    }))
    .expect("serial tab");

    let launch = launch_for_tab(&tab, &AppConfig::default());

    assert!(launch.is_restored());
    assert!(!launch.may_resume_agent());
    assert_eq!(launch.saved().id, "serial-1");
    assert_eq!(
        launch.saved().saved_field("futureTabField"),
        Some(&serde_json::json!({"kept": true}))
    );
    assert!(
        launch
            .connection_options()
            .expect_err("unsupported tab")
            .contains("not supported")
    );
}

#[test]
fn unknown_local_shells_and_control_character_directories_are_not_started() {
    let unknown = launch_for_tab(
        &local_tab(serde_json::json!({
            "type": "terminal",
            "terminalShell": "made-up",
            "loginScript": "echo this must not run",
        })),
        &AppConfig::default(),
    );
    assert!(unknown.connection_options().is_err());

    let launch = launch_for_tab(
        &local_tab(serde_json::json!({
            "type": "terminal",
            "terminalShell": "auto",
            "cwd": "folder\nwith-control",
            "loginScript": "echo this must not run",
        })),
        &AppConfig::default(),
    );
    let options = launch.connection_options().expect("safe local shell");
    assert_eq!(options.terminal_shell.as_deref(), Some("auto"));
    assert_eq!(options.cwd, None);
    assert_eq!(
        launch
            .saved()
            .connection
            .as_ref()
            .and_then(|connection| connection.saved_field("loginScript")),
        Some(&serde_json::json!("echo this must not run"))
    );
}

#[test]
fn a_removed_custom_shell_does_not_run_its_saved_command() {
    let launch = launch_for_tab(
        &local_tab(serde_json::json!({
            "type": "terminal",
            "terminalShell": "custom",
            "terminalShellId": "removed-shell",
            "terminalShellCustomPath": "C:\\removed.exe",
            "terminalShellCustomArgs": "--run-anything",
        })),
        &AppConfig::default(),
    );

    assert!(launch.connection_options().is_err());
}

#[test]
fn only_a_bare_launch_uses_the_saved_workspace() {
    assert!(CommandSpec::Shell.uses_saved_workspace());
    assert!(
        !CommandSpec::Program {
            program: "vim".to_string(),
            args: Vec::new(),
        }
        .uses_saved_workspace()
    );
    assert!(
        !CommandSpec::SshProfile {
            selector: "prod".to_string(),
        }
        .uses_saved_workspace()
    );
}

#[test]
fn saved_ids_wrap_at_the_maximum_without_colliding() {
    let taken = [format!("tab-{}", u64::MAX), "tab-1".to_string()];
    let mut next = u64::MAX;

    let id = Prototype::take_saved_id(&mut next, |id| taken.iter().any(|item| item == id));

    assert_eq!(id, "tab-2");
    assert_eq!(next, 3);
}

#[test]
fn unreadable_and_future_workspaces_are_never_written() {
    assert!(!can_save(&WorkspaceLoad::Unreadable {
        reason: "bad layout".to_string(),
    }));
    assert!(!can_save(&WorkspaceLoad::Future { schema_version: 2 }));
    assert!(can_save(&WorkspaceLoad::Empty));
    assert!(snapshot(WorkspaceLoad::Future { schema_version: 2 }).is_none());
}

#[test]
fn workspace_writes_are_flushed_in_order() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let persistence = test_persistence(runtime.handle(), true, move |snapshot| {
        recorded
            .lock()
            .expect("writes")
            .push(snapshot.tabs[0].id.clone());
        Box::pin(async { Ok(()) })
    });

    runtime.block_on(async {
        persistence
            .flush(saved_snapshot("tab-1"))
            .await
            .expect("first save");
        persistence
            .flush(saved_snapshot("tab-2"))
            .await
            .expect("second save");
    });

    assert_eq!(*writes.lock().expect("writes"), ["tab-1", "tab-2"]);
}

#[test]
fn a_disabled_workspace_keeps_state_in_memory_without_writing() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let writes = Arc::new(Mutex::new(0usize));
    let recorded = Arc::clone(&writes);
    let persistence = test_persistence(runtime.handle(), false, move |_| {
        *recorded.lock().expect("writes") += 1;
        Box::pin(async { Ok(()) })
    });

    runtime
        .block_on(persistence.flush(saved_snapshot("tab-1")))
        .expect("disabled flush");

    assert_eq!(*writes.lock().expect("writes"), 0);
    assert_eq!(
        persistence.latest().expect("latest workspace").tabs[0].id,
        "tab-1"
    );
}

#[test]
fn the_quit_fallback_flushes_the_latest_published_workspace() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let persistence = test_persistence(runtime.handle(), true, move |snapshot| {
        recorded
            .lock()
            .expect("writes")
            .push(snapshot.tabs[0].id.clone());
        Box::pin(async { Ok(()) })
    });
    persistence.publish(Some(saved_snapshot("tab-1")));
    persistence.publish(Some(saved_snapshot("tab-2")));

    runtime
        .block_on(persistence.flush_latest())
        .expect("quit flush");

    assert_eq!(*writes.lock().expect("writes"), ["tab-2"]);
}

#[gpui_kit::test]
fn repeated_changes_coalesce_into_one_debounced_write(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let (notified, notifications) = mpsc::channel();
    let (_runtime, root, visual) = test_workspace(cx, Arc::clone(&writes), notified);

    visual.update(|_, cx| {
        root.update(cx, |workspace, cx| {
            assert!(workspace.mark_workspace_dirty(cx));
            assert!(workspace.mark_workspace_dirty(cx));
            assert!(workspace.mark_workspace_dirty(cx));
        });
    });
    visual.run_until_parked();
    visual
        .cx
        .executor()
        .advance_clock(SAVE_DEBOUNCE + Duration::from_millis(1));
    visual.run_until_parked();
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("debounced save");
    visual.run_until_parked();

    assert_eq!(*writes.lock().expect("writes"), ["workspace"]);
}

#[gpui_kit::test]
fn close_flushes_the_workspace_before_geometry_and_removes_the_window(cx: &mut TestAppContext) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let (notified, notifications) = mpsc::channel();
    let (_runtime, root, visual) = test_workspace(cx, Arc::clone(&writes), notified);

    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            assert!(!workspace.begin_close(window, cx));
        });
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    while (writes.lock().expect("writes").len() < 2 || !visual.cx.windows().is_empty())
        && Instant::now() < deadline
    {
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    notifications
        .try_recv()
        .expect("workspace and geometry writes completed");
    assert_eq!(
        *writes.lock().expect("writes"),
        ["workspace", "geometry"],
        "the final workspace write must finish before geometry"
    );
    assert!(visual.cx.windows().is_empty());
}
