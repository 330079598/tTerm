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
    let events: Events = Rc::new(RefCell::new(Vec::new()));
    let events_for_view = events.clone();
    let (probe, visual) = cx.add_window_view(|window, cx| {
        let selected = events_for_view.clone();
        let closed = events_for_view.clone();
        let menu = cx.new(|cx| {
            ContextMenu::new("test-menu", (20.0, 20.0), actions(), DARK, window, cx)
                .on_action(move |action, _, _| selected.borrow_mut().push(action.into()))
                .on_close(move |_, _| closed.borrow_mut().push("closed".into()))
        });
        Probe { menu }
    });
    visual.update(|w, cx| w.draw(cx).clear(cx));
    let menu = visual.read(|cx| probe.read(cx).menu.clone());
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
    let before =
        cx.read(|cx| serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).unwrap());
    let (_, visual) = cx.add_window_view(|window, cx| Themes {
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
