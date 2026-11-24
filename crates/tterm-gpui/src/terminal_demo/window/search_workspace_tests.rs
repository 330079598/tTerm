//! The find shortcut and pane ownership through the production workspace.
//!
//! These tests use the real prototype window and a local shell, but they only
//! exercise the persisted keymap and search-bar ownership. No SSH profile,
//! credential or user configuration is read.

mod ime;
#[cfg(unix)]
mod live_pty;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, VisualTestContext};

use super::*;
use crate::terminal_demo::runtime::test_runtime;

mod reporting;

fn with_workspace(
    cx: &mut TestAppContext,
    config: AppConfig,
    run: impl FnOnce(Entity<Prototype>, &mut VisualTestContext),
) {
    with_command(cx, config, CommandSpec::Shell, run);
}

fn with_command(
    cx: &mut TestAppContext,
    config: AppConfig,
    command: CommandSpec,
    run: impl FnOnce(Entity<Prototype>, &mut VisualTestContext),
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
    let (root, visual) = cx.add_window_view(|_, cx| {
        split_session_tests::make_workspace_with(&runtime, events, command, config, cx)
    });
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| workspace.select_tab(1, window, cx));
    });
    run(root, visual);
    visual.update(|window, _| window.remove_window());
}

fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }
}

fn default_find() -> String {
    if cfg!(target_os = "macos") {
        format!("{}-f", modifier())
    } else {
        format!("{}-shift-f", modifier())
    }
}

fn press(visual: &mut VisualTestContext, chord: &str) {
    visual.update(|window, cx| window.render_frame(cx));
    visual.simulate_keystrokes(chord);
}

fn active_search(root: &Entity<Prototype>, cx: &App) -> Option<Entity<search::SearchBar>> {
    root.read(cx).tabs.active().search_bar.clone()
}

fn search_is_open(root: &Entity<Prototype>, cx: &App) -> bool {
    active_search(root, cx).is_some_and(|bar| bar.read(cx).is_open())
}

#[gpui_kit::test]
fn the_default_find_shortcut_opens_the_active_panes_search(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &default_find());
        visual.update(|_, cx| {
            assert!(search_is_open(&root, cx));
            let bar = active_search(&root, cx).expect("the bar was created");
            assert!(bar.read(cx).is_open());
        });
    });
}

#[gpui_kit::test]
fn a_custom_find_binding_replaces_the_default_and_unbinding_does_nothing(cx: &mut TestAppContext) {
    let mut config = AppConfig::default();
    config
        .keymap
        .bindings
        .insert("terminal.find".into(), Some(vec!["mod+j".into()]));
    with_workspace(cx, config, |root, visual| {
        press(visual, &format!("{}-j", modifier()));
        visual.update(|_, cx| assert!(search_is_open(&root, cx)));
    });

    let mut config = AppConfig::default();
    config.keymap.bindings.insert("terminal.find".into(), None);
    with_workspace(cx, config, |root, visual| {
        press(visual, &default_find());
        visual.update(|_, cx| {
            assert!(
                active_search(&root, cx).is_none(),
                "an unbound action never creates a bar"
            );
        });
    });
}

#[gpui_kit::test]
fn escape_closes_the_search_even_when_the_terminal_has_the_keyboard(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &default_find());
        visual.update(|window, cx| {
            let focus = root.read(cx).tabs.active().view.read(cx).focus_handle();
            focus.focus(window, cx);
        });
        press(visual, "escape");
        visual.update(|_, cx| {
            assert!(!search_is_open(&root, cx));
            assert!(
                root.read(cx)
                    .tabs
                    .active()
                    .view
                    .read(cx)
                    .search_overlay()
                    .is_empty()
            );
        });
    });
}

#[gpui_kit::test]
fn the_find_chord_does_not_open_a_terminal_search_while_settings_is_open(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| workspace.open_settings(window, cx));
        });
        press(visual, &default_find());
        visual.update(|_, cx| {
            assert!(
                active_search(&root, cx).is_none(),
                "settings keeps its own search field"
            );
        });
    });
}

#[gpui_kit::test]
fn switching_panes_closes_the_previous_panes_search_and_its_highlights(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &format!("{}-\\", modifier()));
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                assert!(workspace.tabs.ordered().len() == 2);
                workspace.focus_pane(1, window, cx);
            });
        });
        press(visual, &default_find());
        visual.update(|_, cx| {
            assert!(search_is_open(&root, cx));
        });

        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                workspace.focus_pane(2, window, cx);
            });
        });

        visual.update(|_, cx| {
            let workspace = root.read(cx);
            for (id, tab) in workspace.tabs.ordered() {
                let bar_open = tab
                    .search_bar
                    .as_ref()
                    .is_some_and(|bar| bar.read(cx).is_open());
                assert!(!bar_open, "pane {id} kept a search bar open");
                assert!(
                    tab.view.read(cx).search_overlay().is_empty(),
                    "pane {id} kept stale highlights"
                );
                assert!(
                    tab.view.read(cx).search_marks().is_empty(),
                    "pane {id} kept stale ruler marks"
                );
            }
        });

        press(visual, &default_find());
        visual.update(|window, cx| {
            assert!(search_is_open(&root, cx));
            let workspace = root.read(cx);
            let active = workspace.tabs.active_id();
            let bar = active_search(&root, cx).expect("the new pane owns the bar");
            assert_eq!(bar.read(cx).status(), &search::SearchStatus::Idle);
            assert_eq!(workspace.tabs.active_id(), active);
            let _ = window;
        });
    });
}

#[gpui_kit::test]
fn dragging_the_handle_keeps_the_bar_in_its_pane_and_leaves_the_other_alone(
    cx: &mut TestAppContext,
) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &format!("{}-\\", modifier()));
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| workspace.focus_pane(1, window, cx));
        });
        press(visual, &default_find());
        visual.update(|window, cx| window.render_frame(cx));

        let pane = visual.debug_bounds("workspace-pane-1").expect("first pane");
        let neighbour = visual
            .debug_bounds("workspace-pane-2")
            .expect("second pane");
        let handle = visual
            .debug_bounds(search::HANDLE_SELECTOR)
            .expect("visible handle")
            .center();
        visual.simulate_mouse_down(handle, gpui_kit::MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            gpui_kit::point(
                pane.right() + gpui_kit::px(200.),
                pane.bottom() + gpui_kit::px(200.),
            ),
            gpui_kit::MouseButton::Left,
            Default::default(),
        );
        visual.update(|window, cx| window.render_frame(cx));

        let moved = visual
            .debug_bounds(search::BAR_SELECTOR)
            .expect("moved bar");
        assert_eq!(moved.right(), pane.right() - gpui_kit::px(8.));
        assert_eq!(moved.bottom(), pane.bottom() - gpui_kit::px(8.));
        assert_eq!(
            visual.debug_bounds("workspace-pane-1"),
            Some(pane),
            "the pane itself did not change"
        );
        assert_eq!(
            visual.debug_bounds("workspace-pane-2"),
            Some(neighbour),
            "the neighbouring pane did not move"
        );

        visual.simulate_mouse_up(
            gpui_kit::point(
                pane.right() + gpui_kit::px(200.),
                pane.bottom() + gpui_kit::px(200.),
            ),
            gpui_kit::MouseButton::Left,
            Default::default(),
        );
        visual.update(|_, cx| {
            let workspace = root.read(cx);
            let neighbour_tab = workspace.tabs.get(2).expect("the second tab");
            assert_eq!(neighbour_tab.view.read(cx).model().selection_text(), None);
            assert!(
                neighbour_tab
                    .search_bar
                    .as_ref()
                    .is_none_or(|bar| !bar.read(cx).is_open()),
                "the drag never opened a search in the neighbouring pane"
            );
        });
    });
}

#[gpui_kit::test]
fn switching_panes_while_the_handle_is_down_ends_the_gesture(cx: &mut TestAppContext) {
    with_workspace(cx, AppConfig::default(), |root, visual| {
        press(visual, &format!("{}-\\", modifier()));
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| workspace.focus_pane(1, window, cx));
        });
        press(visual, &default_find());
        visual.update(|window, cx| window.render_frame(cx));

        let bar = visual
            .update(|_, cx| active_search(&root, cx))
            .expect("first pane bar");
        assert!(!visual.update(|_, cx| bar.read(cx).drag_active()));
        let handle = visual
            .debug_bounds(search::HANDLE_SELECTOR)
            .expect("visible handle")
            .center();
        visual.simulate_mouse_down(handle, gpui_kit::MouseButton::Left, Default::default());
        visual.simulate_mouse_move(
            handle + gpui_kit::point(gpui_kit::px(40.), gpui_kit::px(40.)),
            gpui_kit::MouseButton::Left,
            Default::default(),
        );
        visual.update(|_, cx| assert!(bar.read(cx).drag_active()));

        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| workspace.focus_pane(2, window, cx));
        });
        visual.update(|window, cx| window.render_frame(cx));
        visual.update(|_, cx| {
            assert!(!bar.read(cx).is_open(), "the pane switch closed the bar");
            assert!(
                !bar.read(cx).drag_active(),
                "the pane switch dropped the gesture"
            );
        });
        assert!(
            visual.debug_bounds(search::BAR_SELECTOR).is_none(),
            "the closed bar paints nothing"
        );

        visual.simulate_mouse_up(
            handle + gpui_kit::point(gpui_kit::px(40.), gpui_kit::px(40.)),
            gpui_kit::MouseButton::Left,
            Default::default(),
        );
        visual.simulate_mouse_move(
            handle + gpui_kit::point(gpui_kit::px(120.), gpui_kit::px(120.)),
            None,
            Default::default(),
        );
        visual.update(|window, cx| window.render_frame(cx));
        assert!(visual.debug_bounds(search::BAR_SELECTOR).is_none());
    });
}
