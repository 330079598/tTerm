//! Wheel events exercise routing through the view and assert session bytes or history.

use crate::view::input_tests::Harness;
use gpui_kit::{ScrollDelta, ScrollEvent, TestAppContext, point, px};

fn send(h: &Harness, cx: &mut TestAppContext, delta: ScrollDelta) {
    let position = h.view.read_with(cx, |view, _| {
        let origin = view.input_origin.expect("prepared origin");
        point(origin.x + px(1.0), origin.y + px(1.0))
    });
    h.event(
        cx,
        ScrollEvent {
            position,
            delta,
            ..Default::default()
        },
    );
    h.draw(cx);
}

fn pixels(y: f32) -> ScrollDelta {
    ScrollDelta::Pixels(point(px(0.0), px(y)))
}

fn height(h: &Harness, cx: &mut TestAppContext) -> f32 {
    h.view.read_with(cx, |view, _| {
        view.geometry.expect("prepared metrics").cell_height
    })
}

#[gpui_kit::test]
fn line_notches_report_one_sgr_button_in_each_direction(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1002h\x1b[?1006h");
    send(&h, cx, ScrollDelta::Lines(point(0.0, 3.0)));
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<64;1;1M");
    h.bytes.borrow_mut().clear();
    send(&h, cx, ScrollDelta::Lines(point(0.0, -3.0)));
    assert_eq!(&*h.bytes.borrow(), b"\x1b[<65;1;1M");
}

#[gpui_kit::test]
fn line_notches_send_one_alternate_arrow_in_each_direction(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"\x1b[?1049h\x1b[?1007h");
    send(&h, cx, ScrollDelta::Lines(point(0.0, 3.0)));
    assert_eq!(&*h.bytes.borrow(), b"\x1bOA");
    h.bytes.borrow_mut().clear();
    send(&h, cx, ScrollDelta::Lines(point(0.0, -3.0)));
    assert_eq!(&*h.bytes.borrow(), b"\x1bOB");
}

#[gpui_kit::test]
fn line_notches_scroll_history_by_fifty_pixels(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, &b"history\r\n".repeat(200));
    let cell_height = height(&h, cx);
    for notch in 1..=2 {
        send(&h, cx, ScrollDelta::Lines(point(0.0, 3.0)));
        assert_eq!(
            h.terminal.snapshot().display_offset(),
            (50.0 * notch as f32 / cell_height).trunc() as usize
        );
    }
    send(&h, cx, ScrollDelta::Lines(point(0.0, -3.0)));
    assert_eq!(
        h.terminal.snapshot().display_offset(),
        (100.0 / cell_height).trunc() as usize - (50.0 / cell_height).trunc() as usize
    );
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn pixel_events_scroll_history_in_both_directions(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, &b"history\r\n".repeat(200));
    let cell_height = height(&h, cx);
    send(&h, cx, pixels(3.0 * cell_height));
    assert_eq!(h.terminal.snapshot().display_offset(), 3);
    send(&h, cx, pixels(-cell_height));
    assert_eq!(h.terminal.snapshot().display_offset(), 2);
    assert!(h.bytes.borrow().is_empty());
}

#[gpui_kit::test]
fn pixel_application_events_accumulate_reverse_and_emit_at_most_once(cx: &mut TestAppContext) {
    for (modes, up, down) in [
        (
            b"\x1b[?1002h\x1b[?1006h".as_slice(),
            b"\x1b[<64;1;1M".as_slice(),
            b"\x1b[<65;1;1M".as_slice(),
        ),
        (
            b"\x1b[?1049h\x1b[?1007h".as_slice(),
            b"\x1bOA".as_slice(),
            b"\x1bOB".as_slice(),
        ),
    ] {
        let h = Harness::new(cx);
        h.output(cx, modes);
        let cell_height = height(&h, cx);
        send(&h, cx, pixels(0.375 * cell_height));
        send(&h, cx, pixels(0.375 * cell_height));
        assert!(h.bytes.borrow().is_empty());
        send(&h, cx, pixels(0.375 * cell_height));
        assert_eq!(&*h.bytes.borrow(), up);
        h.bytes.borrow_mut().clear();
        send(&h, cx, pixels(0.875 * cell_height));
        assert_eq!(
            &*h.bytes.borrow(),
            up,
            "the remainder survives an application report"
        );
        h.bytes.borrow_mut().clear();
        send(&h, cx, pixels(3.5 * cell_height));
        assert_eq!(&*h.bytes.borrow(), up, "a large event still emits once");
        h.bytes.borrow_mut().clear();
        send(&h, cx, pixels(-0.75 * cell_height));
        assert!(
            h.bytes.borrow().is_empty(),
            "reversing drops the positive remainder"
        );
        send(&h, cx, pixels(-0.375 * cell_height));
        assert_eq!(&*h.bytes.borrow(), down);
    }
}
