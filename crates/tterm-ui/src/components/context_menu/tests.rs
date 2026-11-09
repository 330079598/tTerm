use super::*;
use crate::tokens::palette::{DARK, LIGHT};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, Entity, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, div, point, px, size,
};
use std::cell::RefCell;

fn actions() -> Vec<MenuAction> {
    (0..5)
        .map(|i| MenuAction {
            label: format!("Item {i}").into(),
            action: i.to_string().into(),
            icon: None,
            separator: i == 1,
            disabled: i == 3,
            shortcut: None,
        })
        .collect()
}
struct Probe {
    menu: Entity<ContextMenu>,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.menu.clone())
    }
}
type Events = Rc<RefCell<Vec<String>>>;
fn harness(cx: &mut TestAppContext) -> (Entity<ContextMenu>, &mut VisualTestContext, Events) {
    cx.update(gpui_kit::init);
    cx.update(crate::init);
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let events_for_view = events.clone();
    let (root, visual) = cx.add_window_view(|window, cx| {
        let probe = cx.new(|cx| {
            let selected = events_for_view.clone();
            let closed = events_for_view.clone();
            let menu = cx.new(|cx| {
                ContextMenu::new("test-menu", (20.0, 20.0), actions(), DARK, window, cx)
                    .on_action(move |action, _, _| selected.borrow_mut().push(action.into()))
                    .on_close(move |_, _| closed.borrow_mut().push("closed".into()))
            });
            Probe { menu }
        });
        gpui_kit::base::Root::new(probe, window, cx)
    });
    visual.update(|w, cx| w.draw(cx).clear(cx));
    let menu = visual.read(|cx| {
        root.read(cx)
            .view()
            .clone()
            .downcast::<Probe>()
            .unwrap()
            .read(cx)
            .menu
            .clone()
    });
    (menu, visual, events)
}
fn state(menu: &Entity<ContextMenu>, visual: &mut VisualTestContext) -> (bool, Option<usize>) {
    visual.read(|cx| (menu.read(cx).is_open(), menu.read(cx).highlighted()))
}
fn redraw(visual: &mut VisualTestContext) {
    visual.update(|w, cx| w.draw(cx).clear(cx));
}

#[gpui_kit::test]
fn keys_wrap_skip_disabled_and_separators_then_enter_calls_action_before_close(
    cx: &mut TestAppContext,
) {
    let (menu, visual, events) = harness(cx);
    assert_eq!(state(&menu, visual), (true, Some(0)));
    for (key, expected) in [("down", 2), ("down", 4), ("down", 0), ("up", 4), ("up", 2)] {
        visual.simulate_keystrokes(key);
        redraw(visual);
        assert_eq!(state(&menu, visual), (true, Some(expected)));
    }
    visual.simulate_keystrokes("enter");
    redraw(visual);
    assert!(!state(&menu, visual).0);
    assert_eq!(&*events.borrow(), &["2", "closed"]);
    visual.simulate_keystrokes("enter");
    assert_eq!(events.borrow().len(), 2);
}
#[gpui_kit::test]
fn escape_and_click_outside_close_without_action(cx: &mut TestAppContext) {
    for escape in [true, false] {
        let (menu, visual, events) = harness(cx);
        if escape {
            visual.simulate_keystrokes("escape");
        } else {
            visual.simulate_click(point(px(500.0), px(300.0)), Modifiers::default());
        }
        redraw(visual);
        assert!(!state(&menu, visual).0);
        assert_eq!(&*events.borrow(), &["closed"]);
    }
}
#[gpui_kit::test]
fn pointer_activation_closes_once_and_disabled_click_does_nothing(cx: &mut TestAppContext) {
    let (menu, visual, events) = harness(cx);
    // Card: border 1 + top 4 + body top 4. Row 3 follows two rows and a separator.
    visual.simulate_click(
        point(px(60.0), px(20.0 + 9.0 + 28.0 * 2.0 + 9.0 + 14.0)),
        Modifiers::default(),
    );
    redraw(visual);
    assert!(events.borrow().is_empty());
    assert!(state(&menu, visual).0);
    visual.simulate_click(point(px(60.0), px(42.0)), Modifiers::default());
    redraw(visual);
    assert_eq!(&*events.borrow(), &["0", "closed"]);
    assert!(!state(&menu, visual).0);
}

#[gpui_kit::test]
fn hover_tracks_enabled_rows_without_changing_keyboard_selection(cx: &mut TestAppContext) {
    let (menu, visual, _) = harness(cx);
    visual.simulate_mouse_move(point(px(60.0), px(80.0)), None, Modifiers::default());
    redraw(visual);
    visual.read(|cx| {
        assert_eq!(menu.read(cx).hovered, Some(2));
        assert_eq!(menu.read(cx).highlighted(), Some(0));
    });
    visual.simulate_mouse_move(point(px(60.0), px(110.0)), None, Modifiers::default());
    redraw(visual);
    visual.read(|cx| assert_eq!(menu.read(cx).hovered, None));
}
#[test]
fn right_bottom_and_corner_use_measured_size_with_ten_pixel_inset() {
    let menu = size(px(256.0), px(199.0));
    let viewport = size(px(800.0), px(600.0));
    for (origin, expected) in [
        ((760.0, 40.0), (534.0, 40.0)),
        ((40.0, 550.0), (40.0, 391.0)),
        ((760.0, 550.0), (534.0, 391.0)),
        ((544.0, 401.0), (544.0, 401.0)),
        ((-5.0, -7.0), (10.0, 10.0)),
    ] {
        assert_eq!(
            placement::fit_in_viewport(point(px(origin.0), px(origin.1)), menu, viewport),
            point(px(expected.0), px(expected.1))
        );
    }
    assert_eq!(
        placement::fit_in_viewport(
            point(px(90.0), px(90.0)),
            size(px(300.0), px(200.0)),
            size(px(100.0), px(80.0))
        ),
        point(px(10.0), px(10.0))
    );
}
struct Themes {
    menus: Vec<Entity<ContextMenu>>,
}
impl Render for Themes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.menus.clone())
    }
}
#[gpui_kit::test]
fn rendering_two_themes_in_one_window_preserves_global_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(crate::init);
    let before =
        cx.read(|cx| serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).unwrap());
    let (_, visual) = cx.add_window_view(|window, cx| {
        let themes = cx.new(|cx| Themes {
            menus: [DARK, LIGHT]
                .into_iter()
                .enumerate()
                .map(|(i, theme)| {
                    cx.new(|cx| {
                        ContextMenu::new(
                            format!("theme-{i}"),
                            (20.0 + i as f32 * 300.0, 20.0),
                            actions(),
                            theme,
                            window,
                            cx,
                        )
                    })
                })
                .collect(),
        });
        gpui_kit::base::Root::new(themes, window, cx)
    });
    visual.update(|window, cx| {
        window.render_frame(cx);
        let quads = window.painted_quads();
        for theme in [DARK, LIGHT] {
            let color = crate::components::surface::css_color(theme.colors.popover, 1.0);
            assert!(
                quads.iter().any(|quad| quad.background == color.into()),
                "both themes must really paint"
            );
        }
    });
    let after = visual
        .read(|cx| serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).unwrap());
    assert_eq!(before, after);
}

#[gpui_kit::test]
fn root_tab_shift_tab_home_end_and_space_stay_in_menu(cx: &mut TestAppContext) {
    let (menu, visual, events) = harness(cx);
    for (key, index) in [
        ("tab", 2),
        ("tab", 4),
        ("shift-tab", 2),
        ("end", 4),
        ("home", 0),
    ] {
        visual.simulate_keystrokes(key);
        redraw(visual);
        assert_eq!(state(&menu, visual), (true, Some(index)));
        visual.update(|window, cx| assert!(menu.read(cx).focus.is_focused(window)));
    }
    visual.simulate_keystrokes("space");
    assert_eq!(&*events.borrow(), &["0", "closed"]);
}

#[gpui_kit::test]
fn callbacks_can_read_and_update_the_menu_after_entity_borrow_ends(cx: &mut TestAppContext) {
    let (menu, visual, events) = harness(cx);
    let selected = menu.clone();
    let closed = menu.clone();
    let action_events = events.clone();
    let close_events = events.clone();
    menu.update(visual, |this, _| {
        this.on_action = Rc::new(move |_, _, cx| {
            assert!(!selected.read(cx).is_open());
            selected.update(cx, |this, _| this.hovered = None);
            action_events.borrow_mut().push("read-action".into());
        });
        this.on_close = Rc::new(move |_, cx| {
            assert!(!closed.read(cx).is_open());
            close_events.borrow_mut().push("read-close".into());
        });
    });
    visual.simulate_keystrokes("enter");
    assert_eq!(&*events.borrow(), &["read-action", "read-close"]);
}

fn painted_ring_count(visual: &mut VisualTestContext) -> usize {
    visual.update(|window, cx| {
        window.render_frame(cx);
        let ring = crate::components::surface::css_color(DARK.colors.ring, 0.5);
        window
            .painted_quads()
            .iter()
            .filter(|quad| quad.border_color == ring)
            .count()
    })
}

#[gpui_kit::test]
fn keyboard_ring_survives_mouse_move_until_pointer_press(cx: &mut TestAppContext) {
    let (_, visual, _) = harness(cx);
    assert_eq!(painted_ring_count(visual), 0);
    visual.simulate_keystrokes("down");
    assert_eq!(painted_ring_count(visual), 1);
    visual.simulate_mouse_move(point(px(60.0), px(80.0)), None, Modifiers::default());
    assert_eq!(painted_ring_count(visual), 1);
    // Card padding clears focus-visible without closing or activating a row.
    visual.simulate_click(point(px(22.0), px(22.0)), Modifiers::default());
    assert_eq!(painted_ring_count(visual), 0);
}

#[gpui_kit::test]
fn hovered_background_survives_keyboard_navigation(cx: &mut TestAppContext) {
    let (_, visual, _) = harness(cx);
    visual.simulate_mouse_move(point(px(60.0), px(42.0)), None, Modifiers::default());
    let hover_color = crate::components::surface::css_color(DARK.colors.accent, 0.5);
    for key in [None, Some("down")] {
        if let Some(key) = key {
            visual.simulate_keystrokes(key);
        }
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|quad| quad.background == hover_color.into())
            );
        });
    }
}

#[gpui_kit::test]
fn explicit_init_restores_menu_bindings_after_keymap_reload(cx: &mut TestAppContext) {
    let (menu, visual, _) = harness(cx);
    visual.update(|_, cx| {
        cx.clear_key_bindings();
        crate::init(cx);
    });
    for (key, index) in [("tab", 2), ("shift-tab", 0), ("down", 2)] {
        visual.simulate_keystrokes(key);
        redraw(visual);
        assert_eq!(state(&menu, visual), (true, Some(index)));
    }
    visual.simulate_keystrokes("escape");
    assert!(!state(&menu, visual).0);
}

gpui_kit::actions!(menu_test, [AncestorCancel]);
struct OverlayProbe {
    menu: Entity<ContextMenu>,
    behind: Rc<std::cell::Cell<usize>>,
    ancestor: Rc<std::cell::Cell<usize>>,
}
impl Render for OverlayProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::{InteractiveElement as _, MouseButton};
        let behind = self.behind.clone();
        let ancestor = self.ancestor.clone();
        div()
            .size_full()
            .key_context("MenuTestDialog")
            .on_action(move |_: &AncestorCancel, _, _| ancestor.set(ancestor.get() + 1))
            .child(
                div()
                    .size(px(12.0))
                    .overflow_hidden()
                    .child(self.menu.clone()),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        behind.set(behind.get() + 1)
                    }),
            )
    }
}
#[gpui_kit::test]
fn deferred_menu_escapes_clipping_occludes_later_sibling_and_owns_escape(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(crate::init);
    cx.update(|cx| {
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "escape",
            AncestorCancel,
            Some("MenuTestDialog"),
        )])
    });
    let behind = Rc::new(std::cell::Cell::new(0));
    let ancestor = Rc::new(std::cell::Cell::new(0));
    let calls = Rc::new(std::cell::Cell::new(0));
    let (root, visual) = cx.add_window_view(|window, cx| {
        let probe = cx.new(|cx| OverlayProbe {
            menu: cx.new(|cx| {
                let calls = calls.clone();
                ContextMenu::new("overlay", (20.0, 20.0), actions(), DARK, window, cx)
                    .on_action(move |_, _, _| calls.set(calls.get() + 1))
            }),
            behind: behind.clone(),
            ancestor: ancestor.clone(),
        });
        gpui_kit::base::Root::new(probe, window, cx)
    });
    redraw(visual);
    visual.simulate_click(point(px(60.0), px(42.0)), Modifiers::default());
    assert_eq!(
        calls.get(),
        1,
        "the row outside the clipped 12px host must receive its click"
    );
    assert_eq!(behind.get(), 0, "the later sibling must be occluded");
    visual.update(|window, cx| {
        let probe = root
            .read(cx)
            .view()
            .clone()
            .downcast::<OverlayProbe>()
            .unwrap();
        probe.read(cx).menu.clone().update(cx, |menu, cx| {
            menu.open = true;
            menu.focus.focus(window, cx);
            cx.notify();
        });
    });
    redraw(visual);
    visual.simulate_keystrokes("escape");
    assert_eq!(ancestor.get(), 0);
    visual.read(|cx| {
        let probe = root
            .read(cx)
            .view()
            .clone()
            .downcast::<OverlayProbe>()
            .unwrap();
        assert!(!probe.read(cx).menu.read(cx).is_open());
    });
}

#[gpui_kit::test]
fn measured_placement_clamps_once_and_only_repositions_when_origin_changes(
    cx: &mut TestAppContext,
) {
    let (menu, visual, _) = harness(cx);
    let bounds = Rc::new(RefCell::new(None));
    menu.update(visual, |menu, cx| {
        let bounds = bounds.clone();
        menu.observer = Some(Rc::new(move |slot, rect, _| {
            if slot.is_empty() {
                *bounds.borrow_mut() = Some(rect);
            }
        }));
        menu.set_origin((10000.0, 10000.0), cx);
    });
    redraw(visual);
    let initial = bounds.borrow().unwrap();
    visual.update(|window, _| {
        assert_eq!(
            initial.origin,
            placement::fit_in_viewport(
                point(px(10000.0), px(10000.0)),
                initial.size,
                window.viewport_size()
            )
        );
    });
    visual.simulate_resize(size(px(400.0), px(300.0)));
    redraw(visual);
    assert_eq!(bounds.borrow().unwrap().origin, initial.origin);
    menu.update(visual, |menu, cx| menu.set_origin((9999.0, 9999.0), cx));
    redraw(visual);
    let moved = bounds.borrow().unwrap();
    assert_eq!(
        moved.origin,
        placement::fit_in_viewport(
            point(px(9999.0), px(9999.0)),
            moved.size,
            size(px(400.0), px(300.0))
        )
    );
}

#[gpui_kit::test]
fn chinese_label_wraps_and_expands_row_from_its_minimum_height(cx: &mut TestAppContext) {
    let (menu, visual, _) = harness(cx);
    let parts = Rc::new(RefCell::new(std::collections::BTreeMap::new()));
    menu.update(visual, |menu, cx| {
        menu.actions.truncate(1);
        menu.actions[0].label =
            "关闭右侧标签页，关闭右侧标签页，关闭右侧标签页，关闭右侧标签页".into();
        let parts = parts.clone();
        menu.observer = Some(Rc::new(move |slot, bounds, style| {
            parts.borrow_mut().insert(slot.to_owned(), (bounds, style));
        }));
        cx.notify();
    });
    redraw(visual);
    let parts = parts.borrow();
    let (row, _) = parts["item-0"];
    let (label, _) = parts["label-0"];
    assert!(row.size.height > px(28.0));
    assert!(label.size.height > px(16.0));
    assert!(label.right() <= row.right() - px(8.0));
}
