//! Right-click gestures that open the host's Copy/Paste menu.
//!
//! These tests own the view side only: the host handler records the request
//! and the menu itself is covered where it is rendered.

use std::cell::RefCell;
use std::rc::Rc;

use super::TerminalViewConfig;
use super::context_menu::{ContextMenuHandler, ContextMenuRequest};
use super::input_tests::Harness;
use gpui_kit::{
    ClipboardItem, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, point, px,
};

type Seen = Rc<RefCell<Option<ContextMenuRequest>>>;

fn with_menu(cx: &mut TestAppContext, right_click_paste: bool) -> (Harness, Seen) {
    let seen: Seen = Rc::new(RefCell::new(None));
    let handler_seen = Rc::clone(&seen);
    let handler: ContextMenuHandler = Rc::new(move |request, _, _| {
        *handler_seen.borrow_mut() = Some(request);
    });
    let h = Harness::with_view_config(
        cx,
        TerminalViewConfig {
            right_click_paste,
            context_menu: Some(handler),
            ..TerminalViewConfig::default()
        },
    );
    (h, seen)
}

fn position(
    h: &Harness,
    cx: &mut TestAppContext,
    column: f32,
    row: f32,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    h.view.read_with(cx, |view, _| {
        let origin = view.input_origin.expect("prepared origin");
        let metrics = view.geometry.expect("prepared metrics");
        point(
            origin.x + px(column * metrics.cell_width),
            origin.y + px(row * metrics.cell_height),
        )
    })
}

fn press_right(h: &Harness, cx: &mut TestAppContext, column: f32, row: f32, modifiers: Modifiers) {
    let position = position(h, cx, column, row);
    h.event(
        cx,
        MouseDownEvent {
            position,
            button: MouseButton::Right,
            modifiers,
            click_count: 1,
            ..Default::default()
        },
    );
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Default::default()
    }
}

#[gpui_kit::test]
fn shift_right_click_opens_the_menu_while_the_program_reports_the_mouse(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, false);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    press_right(&h, cx, 2.1, 0.5, shift());
    let request = seen.borrow().expect("the menu received the gesture");
    assert!(!request.has_selection);
    assert!(
        h.bytes.borrow().is_empty(),
        "a shift right click must not become a mouse report"
    );
}

#[gpui_kit::test]
fn regular_right_click_opens_the_menu_when_right_click_paste_is_off(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, false);
    press_right(&h, cx, 2.1, 0.5, Modifiers::default());
    assert!(seen.borrow().is_some(), "the local click asked for a menu");
    assert!(h.bytes.borrow().is_empty());
    assert!(
        !h.terminal.has_selection(),
        "opening the menu must not start a right-drag selection"
    );
}

#[gpui_kit::test]
fn an_unmodified_right_click_still_belongs_to_a_reporting_program(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, false);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    press_right(&h, cx, 2.1, 0.5, Modifiers::default());
    assert!(seen.borrow().is_none(), "reporting wins over the menu");
    assert!(
        !h.bytes.borrow().is_empty(),
        "the program still receives its right-button press"
    );
}

#[gpui_kit::test]
fn right_click_paste_keeps_precedence_unless_shift_is_held(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, true);
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("lane\r\nnext".into())));
    press_right(&h, cx, 2.1, 0.5, Modifiers::default());
    assert!(seen.borrow().is_none(), "paste does not open the menu");
    assert_eq!(&*h.bytes.borrow(), b"lane\rnext");

    h.bytes.borrow_mut().clear();
    press_right(&h, cx, 2.1, 0.5, shift());
    assert!(
        seen.borrow().is_some(),
        "Shift still asks for the menu when paste is on"
    );
    assert!(h.bytes.borrow().is_empty(), "Shift must not paste");
}

#[gpui_kit::test]
fn the_request_reports_the_selection_that_copy_would_use(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, false);
    h.output(cx, b"hello world");
    let start = position(&h, cx, 0.1, 0.5);
    let end = position(&h, cx, 4.9, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        },
    );
    h.event(
        cx,
        MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            click_count: 1,
        },
    );
    press_right(&h, cx, 2.1, 0.5, Modifiers::default());
    let request = seen.borrow().expect("the menu received the gesture");
    assert!(request.has_selection, "the request saw the copied text");
    assert_eq!(h.terminal.selection_text().as_deref(), Some("hello"));
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn a_menu_gesture_does_not_start_a_right_drag_selection(cx: &mut TestAppContext) {
    let (h, seen) = with_menu(cx, false);
    h.output(cx, b"hello world");
    let start = position(&h, cx, 2.1, 0.5);
    let end = position(&h, cx, 7.9, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            position: start,
            button: MouseButton::Right,
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Right),
            modifiers: Modifiers::default(),
        },
    );
    h.event(
        cx,
        MouseUpEvent {
            position: end,
            button: MouseButton::Right,
            modifiers: Modifiers::default(),
            click_count: 1,
        },
    );
    assert!(seen.borrow().is_some());
    assert!(
        !h.terminal.has_selection(),
        "the click belongs to the menu once it is installed"
    );
    assert!(h.bytes.borrow().is_empty());
}
