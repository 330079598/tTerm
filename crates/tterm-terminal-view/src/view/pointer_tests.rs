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
        "\x1b[200~中文\rnext\t\x1b[201~".as_bytes()
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
            delta: gpui_kit::ScrollDelta::Lines(point(0., 1.)),
            modifiers: Modifiers {
                shift: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<68;3;1M");
}

#[gpui_kit::test]
fn releasing_outside_finishes_local_selection(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world");
    for button in [MouseButton::Left, MouseButton::Right] {
        drag(&h, cx, Modifiers::default());
        let start = position(&h, cx, 4.9, 0.5);
        h.event(
            cx,
            MouseDownEvent {
                position: start,
                button,
                click_count: 1,
                ..Default::default()
            },
        );
        let outside = position(&h, cx, 10000., 0.5);
        h.event(
            cx,
            MouseMoveEvent {
                position: outside,
                pressed_button: Some(button),
                ..Default::default()
            },
        );
        h.event(
            cx,
            MouseUpEvent {
                position: outside,
                button,
                click_count: 1,
                ..Default::default()
            },
        );
        let selected = h.terminal.selection_text();
        assert!(selected.is_some());
        h.event(
            cx,
            MouseMoveEvent {
                position: start,
                pressed_button: None,
                ..Default::default()
            },
        );
        assert_eq!(h.terminal.selection_text(), selected);
    }
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn releasing_outside_pairs_every_reported_button_once(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    for (button, code) in [
        (MouseButton::Left, 0),
        (MouseButton::Middle, 1),
        (MouseButton::Right, 2),
    ] {
        h.bytes.borrow_mut().clear();
        let start = position(&h, cx, 2.1, 0.5);
        h.event(
            cx,
            MouseDownEvent {
                position: start,
                button,
                click_count: 1,
                ..Default::default()
            },
        );
        let release = MouseUpEvent {
            position: point(px(-100.), px(-100.)),
            button,
            click_count: 1,
            ..Default::default()
        };
        h.event(cx, release.clone());
        h.event(cx, release);
        let returned = position(&h, cx, 5.1, 0.5);
        h.event(
            cx,
            MouseMoveEvent {
                position: returned,
                pressed_button: None,
                ..Default::default()
            },
        );
        assert_eq!(
            &*h.bytes.borrow(),
            format!("\x1b[<{code};3;1M\x1b[<{code};1;1m").as_bytes()
        );
    }
}

#[gpui_kit::test]
fn returning_without_a_button_recovers_a_missed_native_release(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world");
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
    let returned = position(&h, cx, 4.9, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: returned,
            pressed_button: None,
            ..Default::default()
        },
    );
    let returned = position(&h, cx, 10.9, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: returned,
            pressed_button: None,
            ..Default::default()
        },
    );
    assert_eq!(h.terminal.selection_text().as_deref(), Some("hello"));
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    for (button, code) in [
        (MouseButton::Left, 0),
        (MouseButton::Middle, 1),
        (MouseButton::Right, 2),
    ] {
        h.bytes.borrow_mut().clear();
        h.event(
            cx,
            MouseDownEvent {
                position: start,
                button,
                click_count: 1,
                ..Default::default()
            },
        );
        let returned = position(&h, cx, 2.1, 0.5);
        h.event(
            cx,
            MouseMoveEvent {
                position: returned,
                pressed_button: None,
                ..Default::default()
            },
        );
        let returned = position(&h, cx, 3.1, 0.5);
        h.event(
            cx,
            MouseMoveEvent {
                position: returned,
                pressed_button: None,
                ..Default::default()
            },
        );
        assert_eq!(
            &*h.bytes.borrow(),
            format!("\x1b[<{code};1;1M\x1b[<{code};3;1m").as_bytes()
        );
    }
}

fn press(
    h: &Harness,
    cx: &mut TestAppContext,
    column: f32,
    row: f32,
    button: MouseButton,
    modifiers: Modifiers,
    click_count: usize,
) {
    let position = position(h, cx, column, row);
    h.event(
        cx,
        MouseDownEvent {
            position,
            button,
            modifiers,
            click_count,
            ..Default::default()
        },
    );
}

fn release(
    h: &Harness,
    cx: &mut TestAppContext,
    column: f32,
    row: f32,
    button: MouseButton,
    modifiers: Modifiers,
    click_count: usize,
) {
    let position = position(h, cx, column, row);
    h.event(
        cx,
        MouseUpEvent {
            position,
            button,
            modifiers,
            click_count,
        },
    );
}

fn click(
    h: &Harness,
    cx: &mut TestAppContext,
    column: f32,
    row: f32,
    click_count: usize,
    modifiers: Modifiers,
) {
    press(
        h,
        cx,
        column,
        row,
        MouseButton::Left,
        modifiers,
        click_count,
    );
    release(
        h,
        cx,
        column,
        row,
        MouseButton::Left,
        modifiers,
        click_count,
    );
}

#[gpui_kit::test]
fn double_click_selects_a_word_with_the_tauri_separators(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"foo:bar|baz<qux> tail");
    click(&h, cx, 1.1, 0.5, 2, Modifiers::default());
    copy(&h, cx);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("foo:bar|baz<qux>".into())
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn triple_click_selects_the_line(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"first line\r\nsecond line");
    click(&h, cx, 2.1, 1.5, 3, Modifiers::default());
    copy(&h, cx);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("second line\n".into())
    );
}

#[gpui_kit::test]
fn alt_drag_selects_a_rectangle(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| {
        view.pointer_policy.alt_selects_block_outside_macos = true;
        view.pointer_policy.macos_option_forces_selection = false;
    });
    h.output(cx, b"abcd\r\nefgh\r\nijkl");
    let modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    press(&h, cx, 0.1, 0.5, MouseButton::Left, modifiers, 1);
    let end = position(&h, cx, 2.9, 1.5);
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
    assert_eq!(h.terminal.selection_text().as_deref(), Some("abc\nefg"));
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn macos_option_forces_selection_when_the_policy_says_so(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| {
        view.pointer_policy.macos_option_forces_selection = true;
        view.pointer_policy.alt_selects_block_outside_macos = false;
    });
    h.output(cx, b"hello world\x1b[?1002h\x1b[?1006h");
    let modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    press(&h, cx, 0.1, 0.5, MouseButton::Left, modifiers, 1);
    let end = position(&h, cx, 4.9, 0.5);
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
    copy(&h, cx);
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("hello".into())
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn alt_without_the_macos_policy_reports_in_mouse_mode(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    // Both platform policies are cleared explicitly: the defaults follow the
    // host, and only a host where Alt belongs to the program reports it.
    h.update(cx, |view, _, _| {
        view.pointer_policy.macos_option_forces_selection = false;
        view.pointer_policy.alt_selects_block_outside_macos = false;
    });
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    let modifiers = Modifiers {
        alt: true,
        ..Default::default()
    };
    click(&h, cx, 1.1, 0.5, 1, modifiers);
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<8;2;1M\x1b[<8;2;1m");
}

#[gpui_kit::test]
fn shift_click_extends_an_existing_selection(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"hello world");
    drag(&h, cx, Modifiers::default());
    assert_eq!(h.terminal.selection_text().as_deref(), Some("hello"));
    click(
        &h,
        cx,
        10.9,
        0.5,
        1,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(h.terminal.selection_text().as_deref(), Some("hello world"));
}

#[gpui_kit::test]
fn linux_middle_click_pastes_the_primary_selection(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| {
        view.pointer_policy.middle_click_pastes = true;
        view.primary = Some(std::rc::Rc::new(|_: &gpui_kit::App| {
            Some("lane\r\nnext".to_string())
        }));
    });
    press(
        &h,
        cx,
        2.1,
        0.5,
        MouseButton::Middle,
        Modifiers::default(),
        1,
    );
    assert_eq!(&*h.bytes.borrow(), b"lane\rnext");
}

#[gpui_kit::test]
fn mode_1000_reports_a_click_in_sgr(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1000h\x1b[?1006h");
    click(&h, cx, 2.1, 1.5, 1, Modifiers::default());
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<0;3;2M\x1b[<0;3;2m");
}

#[gpui_kit::test]
fn mode_1002_reports_a_drag_only_while_a_button_is_held(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    let start = position(&h, cx, 0.1, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: start,
            pressed_button: None,
            modifiers: Modifiers::default(),
        },
    );
    assert!(h.bytes.borrow().is_empty());
    press(&h, cx, 0.1, 0.5, MouseButton::Left, Modifiers::default(), 1);
    let end = position(&h, cx, 2.1, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: end,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        },
    );
    release(&h, cx, 2.1, 0.5, MouseButton::Left, Modifiers::default(), 1);
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<0;1;1M\x1b[<32;3;1M\x1b[<0;3;1m");
}

#[gpui_kit::test]
fn mode_1003_reports_hovering(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1003h\x1b[?1006h");
    let position = position(&h, cx, 2.1, 1.5);
    h.event(
        cx,
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        },
    );
    h.event(
        cx,
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        },
    );
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<35;3;2M");
}

#[gpui_kit::test]
fn mode_1000_without_sgr_uses_the_legacy_encoding(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1000h");
    click(&h, cx, 0.1, 0.5, 1, Modifiers::default());
    assert_eq!(&*h.bytes.borrow(), b"\x1b[M !!\x1b[M#!!");
}

#[gpui_kit::test]
fn copy_on_select_writes_once_on_delivered_and_recovered_release(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| view.copy_on_select = true);
    h.output(cx, b"hello world");
    drag(&h, cx, Modifiers::default());
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("hello".into())
    );
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("keep".into())));
    let end = position(&h, cx, 4.9, 0.5);
    h.event(
        cx,
        MouseUpEvent {
            position: end,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("keep".into()),
        "a second release must not copy again"
    );

    let start = position(&h, cx, 0.1, 0.5);
    h.event(
        cx,
        MouseDownEvent {
            button: MouseButton::Left,
            position: start,
            click_count: 1,
            ..Default::default()
        },
    );
    let returned = position(&h, cx, 4.9, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: returned,
            pressed_button: None,
            ..Default::default()
        },
    );
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("hello".into()),
        "a recovered release copies the final selection"
    );
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("keep".into())));
    let further = position(&h, cx, 5.9, 0.5);
    h.event(
        cx,
        MouseMoveEvent {
            position: further,
            pressed_button: None,
            ..Default::default()
        },
    );
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("keep".into()),
        "a second recovery must not copy again"
    );
}

#[gpui_kit::test]
fn copy_on_select_does_not_replace_the_clipboard_with_an_empty_selection(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| view.copy_on_select = true);
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("keep".into())));
    h.update(cx, |view, _, cx| view.copy_selection_on_release(cx));
    assert_eq!(
        cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("keep".into())
    );
}

#[gpui_kit::test]
fn right_click_pastes_through_the_same_gate_when_enabled(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.update(cx, |view, _, _| view.right_click_paste = true);
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("lane\r\nnext".into())));
    press(
        &h,
        cx,
        2.1,
        0.5,
        MouseButton::Right,
        Modifiers::default(),
        1,
    );
    assert_eq!(&*h.bytes.borrow(), b"lane\rnext");
    assert!(h.terminal.selection_text().is_none());
}
