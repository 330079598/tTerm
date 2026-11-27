use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, VisualTestContext};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::events::AppEvent;
use tterm_core::workspace::pane_templates::LayoutShape;
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::super::split_state::Edge;
use super::super::*;
use super::*;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::session::Session;

fn default_shell() -> CommandSpec {
    CommandSpec::Shell
}

fn exec_marker() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = (
        "cmd.exe",
        vec!["/d".into(), "/q".into(), "/k".into(), "echo RUNONCE".into()],
    );
    #[cfg(not(windows))]
    let (program, args) = (
        "/bin/sh",
        vec!["-c".into(), "echo RUNONCE; exec cat".into()],
    );
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    command: CommandSpec,
    cx: &mut Context<Prototype>,
) -> Prototype {
    let config = AppConfig::default();
    let keymap = Rc::new(KeymapRouter::new(&config.keymap, cfg!(target_os = "macos")));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let handle = runtime._runtime.handle();
    let queue = state::SaveQueue::start(handle, Arc::new(|_| Ok(())));
    let geometry = cx.new(|_| WindowGeometry::new(None, Default::default(), queue));
    let config = cx.new(|_| ConfigState::new(config, handle));
    Prototype::new(
        cx,
        PrototypeStartup {
            command,
            workspace: session_workspace::test_startup(handle),
            sessions: Rc::new(SessionManager::new(
                runtime.services.clone(),
                Default::default(),
            )),
            events,
            keyboard: encoder_config(cfg!(target_os = "macos"), false),
            keymap,
            locale,
            config,
            theme: theme::default_theme(),
        },
        geometry,
    )
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
        let session = tab.binding.as_ref().unwrap().session().unwrap();
        let view = tab.view.read(cx);
        assert_eq!(view.is_visible(), visible.contains(&id));
        assert_eq!(session.is_visible(), view.is_visible());
        assert_eq!(view.focus_handle().is_focused(window), id == focused);
    }
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
    let deadline = Instant::now() + Duration::from_secs(15);
    while !output(model).contains(marker) {
        assert!(
            Instant::now() < deadline,
            "focused PTY did not receive {marker}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn shape(orientation: SplitOrientation, panes: usize) -> LayoutShape {
    LayoutShape::Split {
        orientation,
        children: vec![LayoutShape::Pane; panes],
        sizes: vec![1. / panes as f64; panes],
    }
}

fn workspace(
    cx: &mut TestAppContext,
    command: CommandSpec,
) -> (GpuiRuntime, Entity<Prototype>, &mut VisualTestContext) {
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
    let (root, visual) = cx.add_window_view(|_, cx| make_workspace(&runtime, events, command, cx));
    (runtime, root, visual)
}

fn sessions(workspace: &Prototype, ids: &[usize]) -> Vec<Arc<Session>> {
    ids.iter()
        .map(|id| {
            workspace
                .tabs
                .get(*id)
                .unwrap()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
        })
        .collect()
}

#[gpui_kit::test]
fn same_pane_count_reuses_sessions_and_keeps_the_active_pane(cx: &mut TestAppContext) {
    let (_runtime, root, visual) = workspace(cx, default_shell());
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, default_shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            let before = sessions(workspace, &[1, 2]);

            assert!(workspace.apply_layout_template(
                &shape(SplitOrientation::Vertical, 2),
                window,
                cx
            ));
            check_visible(workspace, &[1, 2], 2, window, cx);
            assert_eq!(workspace.tabs.ordered().len(), 2);
            let after = sessions(workspace, &[1, 2]);
            assert!(
                before
                    .iter()
                    .zip(&after)
                    .all(|(first, second)| Arc::ptr_eq(first, second))
            );
            match workspace.splits.root(2) {
                Some(PaneNode::Split {
                    orientation,
                    children,
                    ..
                }) => {
                    assert_eq!(*orientation, SplitOrientation::Vertical);
                    assert_eq!(children.len(), 2);
                }
                _ => panic!("expected a two-pane column"),
            }
        });
    });
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn smaller_shape_moves_the_surplus_pane_into_its_own_card(cx: &mut TestAppContext) {
    let (_runtime, root, visual) = workspace(cx, default_shell());
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, default_shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.new_tab(window, cx);
            workspace.select_tab(2, window, cx);
            workspace.split_tab(3, 2, Edge::Bottom, window, cx);
            let before = sessions(workspace, &[1, 2, 3]);

            assert!(workspace.apply_layout_template(
                &shape(SplitOrientation::Horizontal, 2),
                window,
                cx
            ));
            check_visible(workspace, &[1, 3], 3, window, cx);
            assert_eq!(
                workspace
                    .tabs
                    .ordered()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>(),
                [1, 2, 3]
            );
            assert_eq!(
                workspace.splits.root(2),
                Some(&PaneNode::Pane {
                    tab_id: "2".to_string(),
                })
            );
            assert!(!workspace.splits.is_split(2));
            let after = sessions(workspace, &[1, 2, 3]);
            assert!(
                before
                    .iter()
                    .zip(&after)
                    .all(|(first, second)| Arc::ptr_eq(first, second))
            );
            assert!(after.iter().all(|session| session.is_live()));
        });
    });
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn reapplying_a_shape_activates_the_target_card(cx: &mut TestAppContext) {
    let (_runtime, root, visual) = workspace(cx, default_shell());
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            let layout = shape(SplitOrientation::Vertical, 2);
            assert!(workspace.apply_layout_template(&layout, window, cx));
            workspace.new_tab(window, cx);
            let before = sessions(workspace, &[1, 2]);

            assert!(workspace.apply_layout_template_to(1, &layout, window, cx));
            check_visible(workspace, &[1, 2], 1, window, cx);
            assert_eq!(workspace.tabs.ordered().len(), 3);
            let after = sessions(workspace, &[1, 2]);
            assert!(
                before
                    .iter()
                    .zip(&after)
                    .all(|(first, second)| Arc::ptr_eq(first, second))
            );
        });
    });
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn larger_shape_opens_live_shells_with_isolated_input(cx: &mut TestAppContext) {
    let (_runtime, root, visual) = workspace(cx, default_shell());
    let models = visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            assert!(workspace.apply_layout_template(
                &shape(SplitOrientation::Vertical, 3),
                window,
                cx
            ));
            check_visible(workspace, &[1, 2, 3], 1, window, cx);
            assert_eq!(
                workspace
                    .tabs
                    .ordered()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>(),
                [1, 2, 3]
            );
            let sessions = sessions(workspace, &[1, 2, 3]);
            assert!(sessions.iter().all(|session| session.is_live()));
            assert!(!Arc::ptr_eq(&sessions[0], &sessions[1]));
            assert!(!Arc::ptr_eq(&sessions[1], &sessions[2]));
            [0, 1, 2].map(|index| sessions[index].model())
        })
    });
    for (index, model) in models.iter().enumerate() {
        let marker = format!("templatepane{index}");
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                workspace.focus_pane(index + 1, window, cx);
            })
        });
        send_marker(visual, model, &marker);
        for (other, candidate) in models.iter().enumerate() {
            if other != index {
                assert!(!output(candidate).contains(&marker));
            }
        }
    }
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn exec_program_is_never_rerun_by_a_larger_template(cx: &mut TestAppContext) {
    let (_runtime, root, visual) = workspace(cx, exec_marker());
    let model = visual.update(|_, cx| {
        root.read(cx)
            .factory
            .sessions
            .tab("gpui-1")
            .unwrap()
            .session()
            .unwrap()
            .model()
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    while !output(&model).contains("RUNONCE") {
        assert!(Instant::now() < deadline, "exec marker did not arrive");
        std::thread::sleep(Duration::from_millis(20));
    }
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.select_tab(1, window, cx);
            let before = sessions(workspace, &[1]);
            assert!(!workspace.apply_layout_template(
                &shape(SplitOrientation::Horizontal, 3),
                window,
                cx
            ));
            assert_eq!(workspace.tabs.ordered().len(), 1);
            check_visible(workspace, &[1], 1, window, cx);
            assert!(Arc::ptr_eq(&before[0], &sessions(workspace, &[1])[0]));
        });
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(output(&model).matches("RUNONCE").count(), 1);
    visual.update(|window, _| window.remove_window());
}
