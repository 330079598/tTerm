use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, VisualTestContext};

use super::split_state::Edge;
use super::*;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};

mod focus_tests;
mod nested_tests;
mod sash_tests;
mod shortcut_tests;

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

fn make_workspace(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    cx: &mut Context<Prototype>,
) -> Prototype {
    make_workspace_with(runtime, events, shell(), AppConfig::default(), cx)
}

pub(super) fn make_workspace_with(
    runtime: &GpuiRuntime,
    events: UnboundedReceiver<AppEvent>,
    command: CommandSpec,
    config: AppConfig,
    cx: &mut Context<Prototype>,
) -> Prototype {
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
            theme: background::theme(None),
            background: background::content(None),
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

#[gpui_kit::test]
fn split_panes_keep_live_sessions_and_route_input_only_to_the_focused_view(
    cx: &mut TestAppContext,
) {
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
    let (root, visual) = cx.add_window_view(|_, cx| make_workspace(&runtime, events, cx));
    let (first, second) = visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            check_visible(workspace, &[1, 2], 2, window, cx);
            let session = |id| {
                workspace
                    .tabs
                    .get(id)
                    .unwrap()
                    .binding
                    .as_ref()
                    .unwrap()
                    .session()
                    .unwrap()
            };
            (session(1), session(2))
        })
    });
    send_marker(visual, &second.model(), "betapane");
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.focus_pane(1, window, cx);
            check_visible(workspace, &[1, 2], 1, window, cx);
        })
    });
    send_marker(visual, &first.model(), "alphapane");
    assert!(!output(&first.model()).contains("betapane"));
    assert!(!output(&second.model()).contains("alphapane"));
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.new_tab(window, cx);
            check_visible(workspace, &[3], 3, window, cx);
            workspace.select_tab(2, window, cx);
            check_visible(workspace, &[1, 2], 2, window, cx);
            let session = |id| {
                workspace
                    .tabs
                    .get(id)
                    .unwrap()
                    .binding
                    .as_ref()
                    .unwrap()
                    .session()
                    .unwrap()
            };
            assert!(Arc::ptr_eq(&first, &session(1)) && Arc::ptr_eq(&second, &session(2)));
            workspace.close_tab(2, window, cx);
            check_visible(workspace, &[1], 1, window, cx);
            assert!(workspace.factory.sessions.tab("gpui-2").is_none());
        })
    });
    send_marker(visual, &first.model(), "survivorpane");
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn active_tab_title_readback_does_not_wait_for_a_render(cx: &mut TestAppContext) {
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
    let (root, visual) = cx.add_window_view(|_, cx| make_workspace(&runtime, events, cx));
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            let first = "first tab title".to_string();
            let second = "second tab title".to_string();
            *workspace
                .tabs
                .get(1)
                .expect("the first tab exists")
                .program_title
                .borrow_mut() = Some(first.clone());
            *workspace
                .tabs
                .get(2)
                .expect("the second tab exists")
                .program_title
                .borrow_mut() = Some(second.clone());
            // Switching reads the new active tab directly; no render occurs here.
            workspace.select_tab(1, window, cx);
            assert_eq!(workspace.logical_window_title(cx), first);
            #[cfg(all(feature = "automation", debug_assertions))]
            assert_eq!(workspace.automation_state(cx), format!("title={first}"));
            workspace.select_tab(2, window, cx);
            assert_eq!(workspace.logical_window_title(cx), second);
            #[cfg(all(feature = "automation", debug_assertions))]
            assert_eq!(workspace.automation_state(cx), format!("title={second}"));
            workspace.select_tab(1, window, cx);
            assert_eq!(workspace.logical_window_title(cx), first);
            #[cfg(all(feature = "automation", debug_assertions))]
            assert_eq!(workspace.automation_state(cx), format!("title={first}"));
        });
    });
    visual.update(|window, _| window.remove_window());
}
