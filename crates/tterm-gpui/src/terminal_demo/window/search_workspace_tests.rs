//! The find shortcut and pane ownership through the production workspace.
//!
//! These tests use the real prototype window and a local shell, but they only
//! exercise the persisted keymap and search-bar ownership. No SSH profile,
//! credential or user configuration is read.

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{TestAppContext, VisualTestContext};

use super::*;
use crate::terminal_demo::runtime::test_runtime;

fn with_workspace(
    cx: &mut TestAppContext,
    config: AppConfig,
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
        split_session_tests::make_workspace_with(&runtime, events, CommandSpec::Shell, config, cx)
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
