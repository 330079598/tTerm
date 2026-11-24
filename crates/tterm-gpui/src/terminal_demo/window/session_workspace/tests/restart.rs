use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui_kit::{App, Entity, TestAppContext, VisualTestContext, Window};
use tterm_core::db::Database;
use tterm_core::session_restore::workspace::{
    WorkspaceLoad, WorkspaceSnapshot, decode_session, encode_session,
};
use tterm_core::session_restore::{read_session, write_session};
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation, pane_leaf};

use super::{make_workspace, test_persistence};
use crate::app_window::state::SaveQueue;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::window::Prototype;
use crate::terminal_demo::window::split_state::Edge;

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(1);

pub(super) struct TestDatabase {
    pub(super) directory: PathBuf,
    path: PathBuf,
}

impl TestDatabase {
    pub(super) fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "tterm-gpui-restart-{}-{}",
            std::process::id(),
            NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("restart database directory");
        let path = directory.join("tterm.db");
        Self { directory, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn open_window<'a>(
    cx: &'a mut TestAppContext,
    database_path: &Path,
    load: WorkspaceLoad,
    restore: bool,
) -> (
    GpuiRuntime,
    Entity<Prototype>,
    &'a mut VisualTestContext,
    mpsc::Receiver<()>,
) {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    let handle = runtime._runtime.handle();
    let (notified, notifications) = mpsc::channel();
    let workspace_path = database_path.to_path_buf();
    let workspace_notified = notified.clone();
    let persistence = test_persistence(handle, true, move |snapshot| {
        let database_path = workspace_path.clone();
        let notified = workspace_notified.clone();
        Box::pin(async move {
            let database = Database::open(&database_path)?;
            let saved_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_millis() as i64);
            database.write(|connection| {
                write_session(connection, encode_session(snapshot, saved_at))
            })?;
            let _ = notified.send(());
            Ok(())
        })
    });
    let geometry_notified = notified;
    let geometry_queue = SaveQueue::start(
        handle,
        Arc::new(move |_| {
            let _ = geometry_notified.send(());
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
    (runtime, root, visual, notifications)
}

fn read_saved_workspace(path: &Path) -> WorkspaceSnapshot {
    let database = Database::open(path).expect("open restart database");
    let session = database
        .read(read_session)
        .expect("read restart session")
        .expect("saved restart session");
    match decode_session(session) {
        WorkspaceLoad::Readable {
            snapshot,
            can_save: true,
        } => snapshot,
        other => panic!("expected a lossless saved workspace, got {other:?}"),
    }
}

fn wait_for_close(visual: &mut VisualTestContext, notifications: &mpsc::Receiver<()>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !visual.cx.windows().is_empty() && Instant::now() < deadline {
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        visual.cx.windows().is_empty(),
        "the restart window did not close"
    );
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("workspace write");
    notifications
        .recv_timeout(Duration::from_secs(2))
        .expect("geometry write");
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "sizes {actual:?} differ from {expected:?}"
        );
    }
}

fn check_visible(
    workspace: &Prototype,
    visible: &[usize],
    focused: usize,
    window: &Window,
    cx: &App,
) {
    assert_eq!(workspace.visible_ids(), visible);
    assert_eq!(workspace.tabs.active_id(), focused);
    for (id, tab) in workspace.tabs.ordered() {
        let session = tab
            .binding
            .as_ref()
            .expect("restored tab binding")
            .session()
            .expect("restored session");
        let view = tab.view.read(cx);
        assert!(session.is_live(), "restored PTY {id} is not live");
        assert_eq!(view.is_visible(), visible.contains(&id));
        assert_eq!(session.is_visible(), view.is_visible());
        assert_eq!(view.focus_handle().is_focused(window), id == focused);
    }
}

#[gpui_kit::test]
fn restart_restores_split_ratio_zoom_and_unique_saved_ids(cx: &mut TestAppContext) {
    let database = TestDatabase::new();

    let saved = {
        let (_runtime, root, visual, notifications) =
            open_window(cx, database.path(), WorkspaceLoad::Empty, false);
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                workspace.new_tab(window, cx);
                let second = workspace.tabs.active_id();
                workspace.select_tab(1, window, cx);
                workspace.split_tab(second, 1, Edge::Right, window, cx);
                assert_eq!(workspace.tabs.active_id(), second);
                assert!(workspace.splits.resize(second, &[], &[0.37, 0.63]));
                let previous = workspace.visible_ids();
                assert!(workspace.splits.toggle_zoom(second));
                workspace.refresh_panes(previous, window, cx);
                workspace.mark_workspace_dirty(cx);
                assert_eq!(
                    workspace
                        .tabs
                        .ordered()
                        .map(|(_, tab)| tab.saved_id())
                        .collect::<Vec<_>>(),
                    ["tab-1", "tab-2"]
                );
                check_visible(workspace, &[second], second, window, cx);
                assert_eq!(workspace.splits.zoomed(second), Some(second));
                assert_close(
                    &workspace
                        .splits
                        .split_sizes(second, &[])
                        .expect("saved root split"),
                    &[0.37, 0.63],
                );
                assert!(!workspace.begin_close(window, cx));
            });
        });
        wait_for_close(visual, &notifications);
        read_saved_workspace(database.path())
    };

    assert_eq!(
        saved
            .tabs
            .iter()
            .map(|tab| tab.id.as_str())
            .collect::<Vec<_>>(),
        ["tab-1", "tab-2"]
    );
    assert_eq!(saved.active_tab_id.as_deref(), Some("tab-2"));
    assert_eq!(saved.groups.len(), 1);
    assert_eq!(saved.groups[0].active_tab_id, "tab-2");
    assert_eq!(saved.groups[0].zoomed_tab_id.as_deref(), Some("tab-2"));
    let PaneNode::Split {
        orientation,
        children,
        sizes,
    } = &saved.groups[0].root
    else {
        panic!("expected the saved root to be a split");
    };
    assert_eq!(*orientation, SplitOrientation::Horizontal);
    assert_eq!(
        children.as_slice(),
        [pane_leaf("tab-1"), pane_leaf("tab-2")].as_slice()
    );
    assert_close(sizes, &[0.37, 0.63]);

    let (_runtime, root, visual, notifications) = open_window(
        cx,
        database.path(),
        WorkspaceLoad::Readable {
            snapshot: saved.clone(),
            can_save: true,
        },
        true,
    );
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.initialize_workspace(window, cx);
            assert_eq!(
                workspace
                    .tabs
                    .ordered()
                    .map(|(_, tab)| tab.saved_id())
                    .collect::<Vec<_>>(),
                ["tab-1", "tab-2"]
            );
            check_visible(workspace, &[2], 2, window, cx);
            assert_eq!(workspace.splits.zoomed(2), Some(2));
            assert_close(
                &workspace
                    .splits
                    .split_sizes(2, &[])
                    .expect("restored root split"),
                &[0.37, 0.63],
            );
            let restored = workspace
                .workspace_snapshot(cx)
                .expect("restored workspace snapshot");
            assert_eq!(restored.groups, saved.groups);
            assert_eq!(restored.active_tab_id, saved.active_tab_id);
            assert_eq!(
                restored
                    .tabs
                    .iter()
                    .map(|tab| tab.id.as_str())
                    .collect::<Vec<_>>(),
                ["tab-1", "tab-2"]
            );
        });
    });

    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            let before = workspace
                .tabs
                .ordered()
                .map(|(_, tab)| tab.saved_id().to_string())
                .collect::<HashSet<_>>();
            workspace.new_tab(window, cx);
            let new_id = workspace.tabs.active().saved_id().to_string();
            assert_eq!(workspace.tabs.len(), 3);
            assert!(!before.contains(&new_id));
            assert_eq!(new_id, "tab-3");
            let ids = workspace
                .tabs
                .ordered()
                .map(|(_, tab)| tab.saved_id().to_string())
                .collect::<Vec<_>>();
            assert_eq!(ids, ["tab-1", "tab-2", "tab-3"]);
            assert!(!workspace.begin_close(window, cx));
        });
    });
    wait_for_close(visual, &notifications);

    let final_saved = read_saved_workspace(database.path());
    let ids = final_saved
        .tabs
        .iter()
        .map(|tab| tab.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["tab-1", "tab-2", "tab-3"]);
    assert_eq!(
        ids.iter().collect::<HashSet<_>>().len(),
        ids.len(),
        "the third tab reused a saved id"
    );
    assert_eq!(final_saved.active_tab_id.as_deref(), Some("tab-3"));
}
