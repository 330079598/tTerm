use super::*;
use crate::tokens::palette::{DARK, LIGHT};
use gpui_kit::{
    AppContext, InteractiveElement, IntoElement, Modifiers, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, div, point, px,
};

use std::cell::RefCell;
use std::collections::HashMap;
struct Probe {
    bounds: Rc<RefCell<HashMap<String, gpui_kit::Bounds<gpui_kit::Pixels>>>>,
    dialog: Entity<Dialog>,
    opener: FocusHandle,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().id("opener").track_focus(&self.opener).child("Open"))
            .child(self.dialog.clone())
    }
}
fn harness(cx: &mut TestAppContext, prevent: bool) -> (Entity<Probe>, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let (probe, visual) = cx.add_window_view(|window, cx| {
        let opener = cx.focus_handle();
        opener.focus(window, cx);
        let bounds = Rc::new(RefCell::new(HashMap::new()));
        let observed = bounds.clone();
        let dialog = cx.new(|cx| {
            let mut options =
                DialogOptions::confirm("Delete profile?", "This action cannot be undone.");
            options.cancel_text = "Cancel".into();
            options.confirm_text = "Confirm".into();
            Dialog::new(options, DARK, window, cx)
                .observe_parts(Rc::new(move |slot, bounds, _| {
                    observed.borrow_mut().insert(slot.to_owned(), bounds);
                }))
                .on_interact_outside(move |event| {
                    if prevent {
                        event.prevent_default();
                    }
                })
        });
        Probe {
            dialog,
            opener,
            bounds,
        }
    });
    redraw(visual);
    (probe, visual)
}
fn redraw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
}
fn dialog(probe: &Entity<Probe>, visual: &mut VisualTestContext) -> Entity<Dialog> {
    visual.read(|cx| probe.read(cx).dialog.clone())
}
fn open(dialog: &Entity<Dialog>, visual: &mut VisualTestContext) -> bool {
    visual.read(|cx| dialog.read(cx).is_open())
}

#[gpui_kit::test]
fn escape_closes_dialog(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    assert!(open(&dialog, visual));
    visual.simulate_keystrokes("escape");
    redraw(visual);
    assert!(!open(&dialog, visual));
}
#[gpui_kit::test]
fn overlay_click_closes_dialog(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    visual.simulate_click(point(px(4.0), px(4.0)), Modifiers::default());
    redraw(visual);
    assert!(!open(&dialog, visual));
}
#[gpui_kit::test]
fn outside_hook_can_prevent_dismissal(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, true);
    let dialog = dialog(&probe, visual);
    visual.simulate_click(point(px(4.0), px(4.0)), Modifiers::default());
    redraw(visual);
    assert!(open(&dialog, visual));
}
#[gpui_kit::test]
fn content_click_keeps_dialog_open(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    let bounds = visual.read(|cx| probe.read(cx).bounds.borrow()["dialog-content"]);
    visual.simulate_click(
        bounds.origin + point(px(30.0), px(30.0)),
        Modifiers::default(),
    );
    redraw(visual);
    assert!(open(&dialog, visual));
}
#[gpui_kit::test]
fn close_button_closes_dialog(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    let bounds = visual.read(|cx| probe.read(cx).bounds.borrow()["dialog-close"]);
    visual.simulate_click(bounds.center(), Modifiers::default());
    redraw(visual);
    assert!(!open(&dialog, visual));
}
#[gpui_kit::test]
fn tab_and_shift_tab_cycle_focus(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    for index in [0, 1, 2, 0] {
        visual.simulate_keystrokes("tab");
        redraw(visual);
        visual.update(|window, cx| assert!(dialog.read(cx).controls[index].is_focused(window)));
    }
    for index in [2, 1, 0, 2] {
        visual.simulate_keystrokes("shift-tab");
        redraw(visual);
        visual.update(|window, cx| assert!(dialog.read(cx).controls[index].is_focused(window)));
    }
    assert!(open(&dialog, visual));
}
#[gpui_kit::test]
fn closing_restores_the_saved_focus_handle(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    visual.simulate_keystrokes("tab escape");
    redraw(visual);
    visual.update(|window, cx| assert!(probe.read(cx).opener.is_focused(window)));
}
#[gpui_kit::test]
fn prompt_tab_cycle_includes_editable_input(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    visual.update(|window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx));
        dialog.update(cx, |dialog, cx| {
            dialog.options.kind = DialogKind::Prompt;
            dialog.input = Some(input);
            dialog.initialized_focus = false;
            cx.notify();
        });
    });
    redraw(visual);
    redraw(visual);
    visual.update(|window, cx| {
        dialog
            .read(cx)
            .input
            .as_ref()
            .expect("input")
            .clone()
            .update(cx, |state, cx| state.focus(window, cx))
    });
    redraw(visual);
    visual.simulate_input("name");
    assert_eq!(
        visual.read(|cx| dialog
            .read(cx)
            .input
            .as_ref()
            .expect("input")
            .read(cx)
            .value()
            .to_string()),
        "name"
    );
    visual.simulate_keystrokes("shift-tab");
    redraw(visual);
    visual.update(|window, cx| assert!(dialog.read(cx).controls[2].is_focused(window)));
    visual.simulate_keystrokes("tab");
    redraw(visual);
    visual.update(|window, cx| {
        assert!(
            dialog
                .read(cx)
                .input
                .as_ref()
                .expect("input")
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        )
    });
}

struct ThemeProbe {
    dark: Entity<Dialog>,
    light: Entity<Dialog>,
}
impl Render for ThemeProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.dark.clone()).child(self.light.clone())
    }
}
#[gpui_kit::test]
fn rendering_two_dialog_themes_does_not_mutate_global_theme(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let before = cx.read(|cx| {
        serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).expect("theme")
    });
    let painted = Rc::new(RefCell::new(Vec::new()));
    let observed = painted.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let dark = cx.new(|cx| {
            Dialog::new(
                DialogOptions::confirm("Dark", "Description"),
                DARK,
                window,
                cx,
            )
            .observe_parts({
                let observed = observed.clone();
                Rc::new(move |slot, bounds, style| {
                    if slot == "dialog-content" {
                        assert!(f32::from(bounds.size.width) > 0.0);
                        observed.borrow_mut().push(style.background);
                    }
                })
            })
        });
        let light = cx.new(|cx| {
            Dialog::new(
                DialogOptions::confirm("Light", "Description"),
                LIGHT,
                window,
                cx,
            )
            .observe_parts({
                let observed = observed.clone();
                Rc::new(move |slot, bounds, style| {
                    if slot == "dialog-content" {
                        assert!(f32::from(bounds.size.width) > 0.0);
                        observed.borrow_mut().push(style.background);
                    }
                })
            })
        });
        ThemeProbe { dark, light }
    });
    redraw(visual);
    assert!(painted.borrow().contains(&DARK.colors.background));
    assert!(painted.borrow().contains(&LIGHT.colors.background));
    let after = visual.read(|cx| {
        serde_json::to_string(gpui_kit::component::theme::Theme::global(cx)).expect("theme")
    });
    assert_eq!(before, after);
}

#[gpui_kit::test]
fn keyboard_activates_each_dialog_button(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    for (index, expected) in [
        (0, Some(DialogAction::Cancel)),
        (1, Some(DialogAction::Confirm)),
        (2, None),
    ] {
        visual.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.reopen(window, cx);
                dialog.initialized_focus = true;
                dialog.controls[index].focus(window, cx);
            })
        });
        redraw(visual);
        visual.simulate_keystrokes("enter");
        redraw(visual);
        assert!(!open(&dialog, visual));
        assert_eq!(visual.read(|cx| dialog.read(cx).last_action), expected);
    }
}
