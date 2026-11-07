//! Selection and clipboard tests exercise the same pointer listeners as the window.

use super::input_tests::Harness;
use gpui_kit::{
    ClipboardItem, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, point, px,
};

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

fn drag(h: &Harness, cx: &mut TestAppContext, modifiers: Modifiers) {
    let start = position(h, cx, 0.1, 0.5);
    let end = position(h, cx, 4.9, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            button: MouseButton::Left,
            position: start,
            modifiers,
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers,
        },
    );
    h.event(
        cx,
        MouseUpEvent {
            button: MouseButton::Left,
            position: end,
            modifiers,
            click_count: 1,
        },
    );
    h.draw(cx);
}

fn copy(h: &Harness, cx: &mut TestAppContext) {
    cx.simulate_keystrokes(
        h.window,
        if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-shift-c"
        },
    );
}

#[gpui_kit::test]
fn dragging_copies_the_selected_text(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world");
    drag(&h, cx, Modifiers::default());
    copy(&h, cx);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("hello".into())
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn shift_drag_selects_instead_of_reporting_to_the_application(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world\x1b[?1002h\x1b[?1006h");
    drag(
        &h,
        cx,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    copy(&h, cx);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("hello".into())
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn mouse_reporting_deduplicates_motion_inside_a_cell(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    let start = position(&h, cx, 0.1, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
    );
    for column in [1.1, 1.2] {
        let position = position(&h, cx, column, 0.5);
        h.event(
            cx,
            MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            },
        );
    }
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<0;1;1M\x1b[<32;2;1M");
}

#[gpui_kit::test]
fn clipboard_paste_normalizes_lines_and_wraps_only_in_bracketed_mode(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("中文\r\nnext\t\x1b\0".into())));
    let paste = if cfg!(target_os = "macos") {
        "cmd-v"
    } else {
        "ctrl-shift-v"
    };
    cx.simulate_keystrokes(h.window, paste);
    assert_eq!(&*h.bytes.borrow(), "中文\rnext\t".as_bytes());
    h.bytes.borrow_mut().clear();
    h.output(cx, b"\x1b[?2004h");
    cx.simulate_keystrokes(h.window, paste);
    assert_eq!(
        &*h.bytes.borrow(),
        "\x1b[200~中文\nnext\t\x1b[201~".as_bytes()
    );
}

#[gpui_kit::test]
fn shift_click_does_not_emit_an_unpaired_release(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello\x1b[?1002h\x1b[?1006h");
    let position = position(&h, cx, 1.1, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn shift_right_drag_extends_selection_but_middle_drag_reports(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world");
    drag(&h, cx, Modifiers::default());
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    let modifiers = Modifiers {
        shift: true,
        ..Default::default()
    };
    let start = position(&h, cx, 4.9, 0.5);
    let end = position(&h, cx, 10.9, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            position: start,
            button: MouseButton::Right,
            modifiers,
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Right),
            modifiers,
        },
    );
    h.event(
        cx,
        MouseUpEvent {
            position: end,
            button: MouseButton::Right,
            modifiers,
            click_count: 1,
        },
    );
    assert_eq!(h.terminal.selection_text().as_deref(), Some("hello world"));
    assert!(h.bytes.borrow().is_empty());
    h.event(
        cx,
        MouseDownEvent {
            position: start,
            button: MouseButton::Middle,
            modifiers,
            click_count: 1,
            ..Default::default()
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Middle),
            modifiers,
        },
    );
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<37;11;1M");
}

#[gpui_kit::test]
fn shift_wheel_keeps_the_existing_reporting_policy(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1000h\x1b[?1006h");
    let position = position(&h, cx, 2.1, 0.5);
    h.event(
        cx,
        gpui_kit::ScrollEvent {
            position,
            delta: gpui_kit::ScrollDelta::Lines(point(0., -1.)),
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<68;3;1M");
}
