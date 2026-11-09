use super::*;
use crate::tokens::palette::{DARK, LIGHT};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, Context, Render, TestAppContext};
use std::cell::Cell;

struct Probe {
    selected: Rc<Cell<usize>>,
    show_both_active: bool,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut root = div().flex().flex_col().p(px(20.0));
        for (index, theme) in [DARK, LIGHT].into_iter().enumerate() {
            let selected = self.selected.clone();
            root = root.child(list(
                theme,
                style(false),
                trigger(
                    format!("tabs-{index}"),
                    "Overview",
                    TriggerState {
                        active: self.show_both_active || selected.get() == index,
                        ..Default::default()
                    },
                    theme,
                    move |_, _, _| selected.set(index),
                ),
            ));
        }
        root.child(content(
            style(false),
            format!("Selected {}", self.selected.get()),
        ))
    }
}

#[gpui_kit::test]
fn pointer_keyboard_and_tab_focus_use_real_buttons(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let selected = Rc::new(Cell::new(0));
    let (_, visual) = cx.add_window_view({
        let selected = selected.clone();
        move |window, cx| {
            let probe = cx.new(|_| Probe {
                selected,
                show_both_active: false,
            });
            gpui_kit::base::Root::new(probe, window, cx)
        }
    });
    visual.update(|window, _| window.activate_window());
    visual.executor().run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        // Root bindings require an initial focus path, just as in gpui-kit's
        // own integration harness. Tab itself then enters each trigger.
        window.click("tabs-1", cx);
        window.press("tab", cx);
        assert_eq!(window.find("tabs-0").focused(), Some(true));
        let blue = css_color(DARK.colors.ring, 1.0);
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|q| q.border_color == blue)
        );
        let first_ring_x = window
            .painted_quads()
            .iter()
            .find(|q| {
                q.border_color == blue
                    && q.border_widths.top.as_f32() == 4.0 * window.scale_factor()
            })
            .expect("first ring")
            .bounds
            .origin
            .y;
        window.press("tab", cx);
        assert_eq!(window.find("tabs-0").focused(), Some(false));
        assert_eq!(window.find("tabs-1").focused(), Some(true));
        let second_ring_y = window
            .painted_quads()
            .iter()
            .find(|q| {
                q.border_color == blue
                    && q.border_widths.top.as_f32() == 4.0 * window.scale_factor()
            })
            .expect("second ring")
            .bounds
            .origin
            .y;
        assert!(
            second_ring_y > first_ring_x,
            "visible ring moves to the next trigger"
        );
        let light_blue = css_color(LIGHT.colors.ring, 1.0);
        assert!(
            window
                .painted_quads()
                .iter()
                .any(|q| q.border_color == light_blue)
        );
        selected.set(0);
        window.render_frame(cx);
        window.press("enter", cx);
        assert_eq!(selected.get(), 1);
        window.click("tabs-0", cx);
        assert_eq!(selected.get(), 0);
        window.press("tab", cx);
        window.press("space", cx);
        assert_eq!(selected.get(), 1);
    });
}

#[gpui_kit::test]
fn two_themes_render_together_without_changing_global_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let before = cx.read(|cx| {
        serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).expect("theme")
    });
    let (_, visual) = cx.add_window_view(|_, _| Probe {
        selected: Rc::new(Cell::new(0)),
        show_both_active: true,
    });
    visual.update(|window, cx| {
        window.render_frame(cx);
        for theme in [DARK, LIGHT] {
            let fill = css_color(theme.colors.background, 1.0);
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|q| q.background == fill.into()),
                "each local theme paints its own active fill"
            );
        }
        assert!(window.find("tabs-0").bounds().size.width > px(0.0));
        assert!(window.find("tabs-1").bounds().size.width > px(0.0));
    });
    let after = visual.read(|cx| {
        serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).expect("theme")
    });
    assert_eq!(before, after);
}
