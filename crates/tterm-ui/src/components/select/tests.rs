use super::*;
use crate::tokens::palette::DARK;
use gpui_kit::{
    AppContext as _, Entity, Modifiers, Point, TestAppContext, VisualTestContext, point,
};
use std::cell::RefCell;

const OPTIONS: [&str; 3] = ["One", "Two", "Three"];
/// The probe pads the select by this much on every side.
const INSET: f32 = 20.0;

struct Probe {
    select: Entity<Select>,
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(INSET))
            .child(div().w(px(300.0)).child(self.select.clone()))
    }
}

fn harness(
    cx: &mut TestAppContext,
    disabled: bool,
) -> (
    Entity<Probe>,
    &mut VisualTestContext,
    Rc<RefCell<Vec<usize>>>,
) {
    cx.update(gpui_kit::init);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    let (probe, visual) = cx.add_window_view(move |_, cx| {
        let select = cx.new(|cx| {
            Select::new(
                "select",
                OPTIONS
                    .iter()
                    .map(|label| SharedString::from(*label))
                    .collect(),
                0,
                DARK,
                cx,
            )
            .disabled(disabled)
        });
        let sink = recorded.clone();
        cx.subscribe(&select, move |_, _, event: &SelectChanged, _| {
            sink.borrow_mut().push(event.0);
        })
        .detach();
        Probe { select }
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
    (probe, visual, changes)
}

fn select_of(probe: &Entity<Probe>, visual: &mut VisualTestContext) -> Entity<Select> {
    visual.read(|cx| probe.read(cx).select.clone())
}

fn state(select: &Entity<Select>, visual: &mut VisualTestContext) -> (bool, usize) {
    visual.read(|cx| {
        let select = select.read(cx);
        (select.is_open(), select.selected())
    })
}

fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(INSET + x), px(INSET + y))
}

/// The centre of the open list's `index`th row: the list starts below the
/// control and the gap, inside its 1px border and 4px padding.
fn option(index: usize) -> Point<Pixels> {
    at(
        20.0,
        HEIGHT + LIST_GAP + 1.0 + 4.0 + 16.0 + 32.0 * index as f32,
    )
}

fn redraw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui_kit::test]
fn clicking_an_option_chooses_it_and_closes_the_list(cx: &mut TestAppContext) {
    let (probe, visual, changes) = harness(cx, false);
    let select = select_of(&probe, visual);
    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    assert_eq!(state(&select, visual), (true, 0));
    visual.simulate_click(option(2), Modifiers::default());
    redraw(visual);
    assert_eq!(state(&select, visual), (false, 2));
    assert_eq!(changes.borrow().as_slice(), &[2]);
}

#[gpui_kit::test]
fn the_keyboard_opens_moves_and_confirms(cx: &mut TestAppContext) {
    let (probe, visual, changes) = harness(cx, false);
    let select = select_of(&probe, visual);
    visual.update(|window, cx| select.read(cx).focus_handle().clone().focus(window, cx));
    redraw(visual);
    visual.simulate_keystrokes("down");
    redraw(visual);
    assert_eq!(state(&select, visual), (true, 0));
    visual.simulate_keystrokes("down down up");
    visual.simulate_keystrokes("enter");
    redraw(visual);
    assert_eq!(state(&select, visual), (false, 1));
    assert_eq!(changes.borrow().as_slice(), &[1]);
}

#[gpui_kit::test]
fn escape_and_outside_clicks_close_without_choosing(cx: &mut TestAppContext) {
    let (probe, visual, changes) = harness(cx, false);
    let select = select_of(&probe, visual);
    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    visual.simulate_keystrokes("down escape");
    redraw(visual);
    assert_eq!(state(&select, visual), (false, 0));

    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    assert!(state(&select, visual).0);
    // Far below the list, inside the window.
    visual.simulate_click(at(250.0, 400.0), Modifiers::default());
    redraw(visual);
    assert_eq!(state(&select, visual), (false, 0));

    // A second press on the control closes the list instead of reopening it.
    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    assert!(!state(&select, visual).0);
    assert!(changes.borrow().is_empty());
}

#[gpui_kit::test]
fn a_disabled_select_does_not_open(cx: &mut TestAppContext) {
    let (probe, visual, _) = harness(cx, true);
    let select = select_of(&probe, visual);
    visual.simulate_click(at(50.0, 18.0), Modifiers::default());
    redraw(visual);
    assert_eq!(state(&select, visual), (false, 0));
}

#[test]
fn the_control_follows_the_css_states() {
    let idle = style(false, false, DARK);
    assert_eq!(idle.border, DARK.colors.input);
    assert_eq!(style(true, false, DARK).border, DARK.colors.ring);
    assert_eq!(style(false, true, DARK).opacity, 0.5);
    assert_eq!(idle.padding, (4.0, 36.0, 4.0, 12.0));
}
