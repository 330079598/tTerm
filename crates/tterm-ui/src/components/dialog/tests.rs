use super::*;
use crate::tokens::palette::{DARK, LIGHT};
use gpui_kit::{
    AppContext, Focusable, InteractiveElement, IntoElement, Modifiers, ParentElement, Render,
    StatefulInteractiveElement, Styled, TestAppContext, VisualTestContext, div, point, px,
};

use std::cell::RefCell;
use std::collections::HashMap;
struct Probe {
    bounds: Rc<RefCell<HashMap<String, gpui_kit::Bounds<gpui_kit::Pixels>>>>,
    styles: Rc<RefCell<HashMap<String, presentation::PartStyle>>>,
    texts: Rc<RefCell<HashMap<String, gpui_kit::Bounds<gpui_kit::Pixels>>>>,
    received: Rc<RefCell<[usize; 3]>>,
    dialog: Entity<Dialog>,
    opener: FocusHandle,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .tab_group()
            .child(
                div()
                    .id("background")
                    .size_full()
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.received.borrow_mut()[0] += 1;
                            this.opener.focus(window, cx);
                        }),
                    )
                    .on_hover(cx.listener(|this, hovered, _, _| {
                        if *hovered {
                            this.received.borrow_mut()[1] += 1;
                        }
                    }))
                    .on_scroll(cx.listener(|this, _, _, _| this.received.borrow_mut()[2] += 1))
                    .child(
                        div()
                            .id("opener")
                            .track_focus(&self.opener)
                            .tab_index(0)
                            .size(gpui_kit::px(24.0))
                            .child("Open"),
                    ),
            )
            // Dialog is an overlay sibling at the window origin, like the
            // production body portal. It must not follow the opener in flow.
            .child(self.dialog.clone())
    }
}
fn options() -> DialogOptions {
    let mut options = DialogOptions::confirm("Delete profile?", "This action cannot be undone.");
    options.cancel_text = "Cancel".into();
    options.confirm_text = "Confirm".into();
    options
}
fn harness(cx: &mut TestAppContext, prevent: bool) -> (Entity<Probe>, &mut VisualTestContext) {
    configured_harness(cx, prevent, options())
}
fn configured_harness(
    cx: &mut TestAppContext,
    prevent: bool,
    options: DialogOptions,
) -> (Entity<Probe>, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let mut built = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let probe = cx.new(|cx| {
            let opener = cx.focus_handle().tab_stop(true);
            opener.focus(window, cx);
            let bounds = Rc::new(RefCell::new(HashMap::new()));
            let styles = Rc::new(RefCell::new(HashMap::new()));
            let texts = Rc::new(RefCell::new(HashMap::new()));
            let observed_texts = texts.clone();
            let observed = bounds.clone();
            let observed_styles = styles.clone();
            let input = (options.kind == DialogKind::Prompt)
                .then(|| cx.new(|cx| InputState::new(window, cx)));
            let dialog = cx.new(|cx| {
                let mut dialog = Dialog::new(options, DARK, window, cx)
                    .observe_parts(Rc::new(move |slot, bounds, style| {
                        observed.borrow_mut().insert(slot.to_owned(), bounds);
                        observed_styles
                            .borrow_mut()
                            .insert(slot.to_owned(), style.clone());
                    }))
                    .observe_text(Rc::new(move |slot, bounds| {
                        observed_texts.borrow_mut().insert(slot.to_owned(), bounds);
                    }))
                    .on_interact_outside(move |event| {
                        if prevent {
                            event.prevent_default();
                        }
                    });
                if let Some(input) = input {
                    dialog = dialog.with_input(input);
                }
                dialog
            });
            Probe {
                dialog,
                opener,
                bounds,
                styles,
                texts,
                received: Rc::new(RefCell::new([0; 3])),
            }
        });
        built = Some(probe.clone());
        gpui_kit::base::Root::new(probe, window, cx)
    });
    visual.update(|window, _| window.activate_window());
    redraw(visual);
    (built.expect("Root built application content"), visual)
}
fn overlay_point(
    probe: &Entity<Probe>,
    visual: &mut VisualTestContext,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    let point = point(px(80.0), px(80.0));
    visual.read(|cx| {
        let probe = probe.read(cx);
        let bounds = probe.bounds.borrow();
        assert_eq!(bounds["dialog-overlay"].origin, gpui_kit::Point::default());
        assert!(bounds["dialog-overlay"].contains(&point));
        assert!(!bounds["dialog-content"].contains(&point));
    });
    point
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
    let point = overlay_point(&probe, visual);
    visual.simulate_click(point, Modifiers::default());
    redraw(visual);
    assert!(!open(&dialog, visual));
    visual.update(|window, cx| {
        assert!(probe.read(cx).opener.is_focused(window));
        assert_eq!(probe.read(cx).received.borrow()[0], 0);
    });
}
#[gpui_kit::test]
fn outside_hook_can_prevent_dismissal(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, true);
    let dialog = dialog(&probe, visual);
    let point = overlay_point(&probe, visual);
    visual.simulate_click(point, Modifiers::default());
    redraw(visual);
    assert!(open(&dialog, visual));
    visual.update(|window, cx| {
        assert!(dialog.read(cx).focus.contains_focused(window, cx));
        assert_eq!(*probe.read(cx).received.borrow(), [0; 3]);
    });
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
    visual.read(|cx| assert_eq!(*probe.read(cx).received.borrow(), [0; 3]));
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
fn prompt_options() -> DialogOptions {
    let mut options = DialogOptions::prompt("Rename profile", "Name");
    options.placeholder = "Profile name".into();
    options.cancel_text = "Cancel".into();
    options.confirm_text = "OK".into();
    options
}
fn assert_input_focused(dialog: &Entity<Dialog>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        assert!(
            dialog
                .read(cx)
                .input
                .as_ref()
                .expect("prompt input")
                .read(cx)
                .focus_handle(cx)
                .is_focused(window)
        );
    });
}
#[gpui_kit::test]
fn prompt_tab_cycle_includes_editable_input(cx: &mut TestAppContext) {
    let (probe, visual) = configured_harness(cx, false, prompt_options());
    let dialog = dialog(&probe, visual);
    assert_input_focused(&dialog, visual);
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
    assert_input_focused(&dialog, visual);
    for index in [0, 1, 2] {
        visual.simulate_keystrokes("tab");
        redraw(visual);
        visual.update(|window, cx| assert!(dialog.read(cx).controls[index].is_focused(window)));
    }
    visual.simulate_keystrokes("tab");
    redraw(visual);
    assert_input_focused(&dialog, visual);
}
#[gpui_kit::test]
fn enter_submits_prompt_from_autofocused_input(cx: &mut TestAppContext) {
    let (probe, visual) = configured_harness(cx, false, prompt_options());
    let dialog = dialog(&probe, visual);
    assert_input_focused(&dialog, visual);
    visual.simulate_input("Production");
    visual.simulate_keystrokes("enter");
    redraw(visual);
    visual.read(|cx| {
        let dialog = dialog.read(cx);
        assert!(!dialog.is_open());
        assert_eq!(dialog.last_action, Some(DialogAction::Confirm));
        assert_eq!(
            dialog
                .input
                .as_ref()
                .expect("input")
                .read(cx)
                .value()
                .as_ref(),
            "Production"
        );
    });
}
#[gpui_kit::test]
fn escape_closes_prompt_from_autofocused_input(cx: &mut TestAppContext) {
    let (probe, visual) = configured_harness(cx, false, prompt_options());
    let dialog = dialog(&probe, visual);
    assert_input_focused(&dialog, visual);
    visual.simulate_keystrokes("escape");
    redraw(visual);
    assert!(!open(&dialog, visual));
    visual.update(|window, cx| assert!(probe.read(cx).opener.is_focused(window)));
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
        let content = cx.new(|_| ThemeProbe { dark, light });
        gpui_kit::base::Root::new(content, window, cx)
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
    for (tabs, expected) in [
        ("tab", Some(DialogAction::Cancel)),
        ("tab tab", Some(DialogAction::Confirm)),
        ("tab tab tab", None),
    ] {
        visual.update(|window, cx| {
            dialog.update(cx, |dialog, cx| dialog.reopen(window, cx));
        });
        redraw(visual);
        visual.simulate_keystrokes(tabs);
        redraw(visual);
        visual.simulate_keystrokes("enter");
        redraw(visual);
        assert!(!open(&dialog, visual));
        assert_eq!(visual.read(|cx| dialog.read(cx).last_action), expected);
    }
}

#[gpui_kit::test]
fn modal_occludes_background_mouse_hover_and_scroll(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, true);
    let overlay = overlay_point(&probe, visual);
    let content = visual.read(|cx| probe.read(cx).bounds.borrow()["dialog-content"].center());
    for position in [overlay, content] {
        visual.update(|window, cx| window.simulate_mouse_move(position, cx));
        visual.simulate_event(gpui_kit::ScrollEvent {
            position,
            delta: gpui_kit::ScrollDelta::Lines(gpui_kit::point(0.0, -3.0)),
            ..Default::default()
        });
        visual.simulate_click(position, Modifiers::default());
        redraw(visual);
    }
    visual.read(|cx| assert_eq!(*probe.read(cx).received.borrow(), [0; 3]));
    // Verify that the probe really observes all three event types when exposed.
    visual.simulate_keystrokes("escape");
    redraw(visual);
    visual.update(|window, cx| window.simulate_mouse_move(point(px(81.0), px(81.0)), cx));
    visual.simulate_event(gpui_kit::ScrollEvent {
        position: overlay,
        ..Default::default()
    });
    visual.simulate_click(overlay, Modifiers::default());
    visual.read(|cx| {
        assert!(
            probe
                .read(cx)
                .received
                .borrow()
                .iter()
                .all(|count| *count > 0)
        )
    });
}
#[gpui_kit::test]
fn default_action_sets_initial_focus_and_overrides_enter(cx: &mut TestAppContext) {
    let mut options = options();
    options.default_action = Some(DialogAction::Cancel);
    let (probe, visual) = configured_harness(cx, false, options);
    let dialog = dialog(&probe, visual);
    visual.update(|window, cx| assert!(dialog.read(cx).controls[0].is_focused(window)));
    visual.simulate_keystrokes("tab");
    redraw(visual);
    visual.update(|window, cx| assert!(dialog.read(cx).controls[1].is_focused(window)));
    visual.simulate_keystrokes("enter");
    assert_eq!(
        visual.read(|cx| dialog.read(cx).last_action),
        Some(DialogAction::Cancel)
    );
}
#[gpui_kit::test]
fn confirm_default_action_focuses_confirm(cx: &mut TestAppContext) {
    let mut options = options();
    options.default_action = Some(DialogAction::Confirm);
    let (probe, visual) = configured_harness(cx, false, options);
    let dialog = dialog(&probe, visual);
    visual.update(|window, cx| assert!(dialog.read(cx).controls[1].is_focused(window)));
    visual.simulate_keystrokes("enter");
    assert_eq!(
        visual.read(|cx| dialog.read(cx).last_action),
        Some(DialogAction::Confirm)
    );
}
#[gpui_kit::test]
fn info_renders_one_action_and_cycles_with_close(cx: &mut TestAppContext) {
    let mut options = DialogOptions::info("Connection ready", "You can continue.");
    options.confirm_text = "OK".into();
    let (probe, visual) = configured_harness(cx, false, options);
    let dialog = dialog(&probe, visual);
    visual.read(|cx| assert!(!probe.read(cx).bounds.borrow().contains_key("cancel")));
    for index in [1, 2, 1] {
        visual.simulate_keystrokes("tab");
        redraw(visual);
        visual.update(|window, cx| assert!(dialog.read(cx).controls[index].is_focused(window)));
    }
    visual.simulate_keystrokes("enter");
    assert!(!open(&dialog, visual));
}
#[gpui_kit::test]
fn prompt_placeholder_and_label_use_public_options(cx: &mut TestAppContext) {
    let (probe, visual) = configured_harness(cx, false, prompt_options());
    visual.read(|cx| {
        let probe = probe.read(cx);
        let style = probe.styles.borrow()["label"].clone();
        assert_eq!(
            (style.font_size, style.line_height, style.weight),
            (14.0, 14.0, 500)
        );
        let input = probe
            .dialog
            .read(cx)
            .input
            .as_ref()
            .expect("input")
            .read(cx);
        assert_eq!(input.presentation().placeholder().as_ref(), "Profile name");
    });
}
#[gpui_kit::test]
fn action_and_close_hover_are_painted_and_observed(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    for slot in ["cancel", "confirm", "dialog-close"] {
        let before = visual.read(|cx| probe.read(cx).styles.borrow()[slot].clone());
        let bounds = visual.read(|cx| probe.read(cx).bounds.borrow()[slot]);
        visual.update(|window, cx| window.simulate_mouse_move(bounds.center(), cx));
        redraw(visual);
        redraw(visual);
        let after = visual.read(|cx| probe.read(cx).styles.borrow()[slot].clone());
        if slot == "dialog-close" {
            assert_eq!(after.opacity, 1.0);
            assert_eq!(before.opacity, 0.7);
        } else {
            assert_ne!(before.background_alpha, after.background_alpha);
        }
        visual.update(|window, cx| window.simulate_mouse_move(point(px(80.0), px(80.0)), cx));
        redraw(visual);
    }
}
#[gpui_kit::test]
fn narrow_window_stacks_footer_and_centers_header(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    visual.simulate_resize(gpui_kit::size(px(500.0), px(600.0)));
    redraw(visual);
    visual.read(|cx| {
        let probe = probe.read(cx);
        let bounds = probe.bounds.borrow();
        assert!(probe.styles.borrow()["dialog-header"].centered);
        assert_eq!(f32::from(bounds["dialog-content"].size.width), 468.0);
        assert_eq!(f32::from(bounds["dialog-content"].origin.x), 16.0);
        assert!(bounds["confirm"].origin.y < bounds["cancel"].origin.y);
        assert_eq!(
            bounds["confirm"].size.width,
            bounds["dialog-footer"].size.width
        );
        assert_eq!(
            bounds["cancel"].size.width,
            bounds["dialog-footer"].size.width
        );
    });
}
#[gpui_kit::test]
fn description_preserves_newlines_and_wraps(cx: &mut TestAppContext) {
    let mut options = options();
    options.description = Some("First line.\nThis much longer second line will wrap automatically because it contains more text than can fit inside the dialog content width.".into());
    let (probe, visual) = configured_harness(cx, false, options);
    visual.read(|cx| {
        let probe = probe.read(cx);
        let bounds = probe.bounds.borrow();
        assert!(f32::from(bounds["dialog-description"].size.height) >= 3.0 * 16.0);
        assert!(bounds["dialog-footer"].origin.y > bounds["dialog-description"].bottom());
    });
}

#[gpui_kit::test]
fn short_description_newline_creates_two_separate_rows(cx: &mut TestAppContext) {
    let mut options = options();
    options.description = Some("Alpha\nBeta".into());
    let (probe, visual) = configured_harness(cx, false, options);
    visual.read(|cx| {
        let probe = probe.read(cx);
        let rows = probe.texts.borrow();
        let first = rows["description-0"];
        let second = rows["description-1"];
        assert_eq!(first.origin.x, second.origin.x);
        assert_eq!(second.origin.y - first.origin.y, px(16.0));
        assert!(first.size.width < px(100.0));
        assert!(second.size.width < px(100.0));
        assert!(!rows.contains_key("description-2"));
        assert_eq!(
            probe.bounds.borrow()["dialog-description"].size.height,
            px(32.0)
        );
    });
}
#[gpui_kit::test]
fn trailing_space_does_not_move_a_fitting_word_to_the_next_row(cx: &mut TestAppContext) {
    let mut options = options();
    options.description = Some("Alpha Beta Gamma".into());
    let (probe, visual) = configured_harness(cx, false, options);
    let width = visual.update(|window, _| {
        let style = gpui_kit::TextStyle {
            font_family: ".SystemUIFont".into(),
            ..Default::default()
        };
        window
            .text_system()
            .shape_line("Alpha Beta".into(), px(14.0), &[style.to_run(10)], None)
            .width
    });
    // Narrow content subtracts 32px margins, 48px padding and 2px border.
    // One extra pixel survives layout rounding while the mock space is wider.
    visual.simulate_resize(gpui_kit::size(width.ceil() + px(83.0), px(600.0)));
    redraw(visual);
    visual.read(|cx| {
        let probe = probe.read(cx);
        let rows = probe.texts.borrow();
        assert_eq!(rows["description-0"].size.width, width);
        assert_eq!(
            rows["description-1"].center().x,
            rows["description-0"].center().x
        );
        assert!(!rows.contains_key("description-2"));
    });
}
fn assert_narrow_title(
    cx: &mut TestAppContext,
    options: DialogOptions,
    centered: bool,
    icon: bool,
) {
    let (probe, visual) = configured_harness(cx, false, options);
    visual.simulate_resize(gpui_kit::size(px(500.0), px(600.0)));
    redraw(visual);
    visual.read(|cx| {
        let probe = probe.read(cx);
        let title = probe.bounds.borrow()["dialog-title"];
        let text = probe.texts.borrow()["title-0"];
        if centered {
            assert!((f32::from(text.center().x - title.center().x)).abs() < 0.01);
        } else {
            assert_eq!(
                text.origin.x,
                title.origin.x + px(if icon { 24.0 } else { 0.0 })
            );
        }
    });
}
#[gpui_kit::test]
fn narrow_confirm_title_stays_left(cx: &mut TestAppContext) {
    assert_narrow_title(cx, options(), false, false);
}
#[gpui_kit::test]
fn narrow_destructive_title_stays_left_after_icon(cx: &mut TestAppContext) {
    let mut options = options();
    options.destructive = true;
    assert_narrow_title(cx, options, false, true);
}
#[gpui_kit::test]
fn narrow_info_title_stays_left_after_icon(cx: &mut TestAppContext) {
    assert_narrow_title(
        cx,
        DialogOptions::info("Information", "Description"),
        false,
        true,
    );
}
#[gpui_kit::test]
fn narrow_prompt_title_is_centered(cx: &mut TestAppContext) {
    assert_narrow_title(cx, prompt_options(), true, false);
}
#[gpui_kit::test]
fn all_mouse_buttons_dismiss_the_overlay(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    for button in [
        gpui_kit::MouseButton::Left,
        gpui_kit::MouseButton::Right,
        gpui_kit::MouseButton::Middle,
    ] {
        visual.update(|window, cx| dialog.update(cx, |dialog, cx| dialog.reopen(window, cx)));
        redraw(visual);
        let position = overlay_point(&probe, visual);
        visual.simulate_mouse_down(position, button, Modifiers::default());
        redraw(visual);
        assert!(!open(&dialog, visual));
    }
}
#[gpui_kit::test]
fn close_and_reopen_clear_hover_state(cx: &mut TestAppContext) {
    let (probe, visual) = harness(cx, false);
    let dialog = dialog(&probe, visual);
    for slot in ["cancel", "confirm", "dialog-close"] {
        let bounds = visual.read(|cx| probe.read(cx).bounds.borrow()[slot]);
        visual.update(|window, cx| window.simulate_mouse_move(bounds.center(), cx));
        redraw(visual);
        redraw(visual);
        visual.read(|cx| assert!(dialog.read(cx).hovered.iter().any(|hover| *hover)));
        visual.simulate_keystrokes("escape");
        visual.read(|cx| assert_eq!(dialog.read(cx).hovered, [false; 3]));
        redraw(visual);
        visual.update(|window, cx| {
            window.simulate_mouse_move(point(px(80.0), px(80.0)), cx);
            dialog.update(cx, |dialog, cx| dialog.reopen(window, cx));
        });
        redraw(visual);
        visual.read(|cx| assert_eq!(dialog.read(cx).hovered, [false; 3]));
    }
}
