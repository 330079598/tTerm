//! Full-history scrolling through the real view events and history clearing.
//!
//! The harness writes synthetic output into the production model, then uses
//! the same wheel and pointer listeners a window uses. Each assertion reads
//! the frame the view prepared, not a separately constructed snapshot.

use super::input_tests::Harness;
use gpui_kit::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollEvent,
    TestAppContext, point, px,
};

const CAPACITY: usize = 6;

fn fill(h: &Harness, cx: &mut TestAppContext, lines: usize) {
    let mut bytes = Vec::with_capacity(lines * 12);
    for index in 0..lines {
        bytes.extend_from_slice(format!("line-{index:02}\r\n").as_bytes());
    }
    h.output(cx, &bytes);
}

fn visible_text(h: &Harness, cx: &mut TestAppContext) -> Vec<String> {
    h.view.read_with(cx, |view, _| {
        let frame = view.last_frame.as_ref().expect("a prepared frame");
        frame
            .snapshot
            .rows()
            .iter()
            .map(|row| {
                row.cells()
                    .iter()
                    .map(|cell| cell.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    })
}

fn display_offset(h: &Harness, cx: &mut TestAppContext) -> usize {
    h.view.read_with(cx, |view, _| {
        view.last_frame
            .as_ref()
            .expect("a prepared frame")
            .snapshot
            .display_offset()
    })
}

fn history_size(h: &Harness, cx: &mut TestAppContext) -> usize {
    h.view.read_with(cx, |view, _| {
        view.last_frame
            .as_ref()
            .expect("a prepared frame")
            .snapshot
            .history_size()
    })
}

fn generation(h: &Harness, cx: &mut TestAppContext) -> u64 {
    h.view.read_with(cx, |view, _| {
        view.last_frame
            .as_ref()
            .expect("a prepared frame")
            .scrollback_generation()
    })
}

fn cell_height(h: &Harness, cx: &mut TestAppContext) -> f32 {
    h.view.read_with(cx, |view, _| {
        view.geometry.expect("prepared metrics").cell_height
    })
}

fn grid_position(h: &Harness, cx: &mut TestAppContext) -> gpui_kit::Point<gpui_kit::Pixels> {
    h.view.read_with(cx, |view, _| {
        let origin = view.input_origin.expect("prepared origin");
        let metrics = view.geometry.expect("prepared metrics");
        point(
            origin.x + px(metrics.cell_width / 2.0),
            origin.y + px(metrics.cell_height / 2.0),
        )
    })
}

fn gutter_position(
    h: &Harness,
    cx: &mut TestAppContext,
    y: f32,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    h.view.read_with(cx, |view, _| {
        let origin = view.input_origin.expect("prepared origin");
        let metrics = view.geometry.expect("prepared metrics");
        let frame = view.last_frame.as_ref().expect("a prepared frame");
        let x = metrics.cell_width * frame.grid().columns as f32 + 5.0;
        point(origin.x + px(x), origin.y + px(y))
    })
}

fn wheel(h: &Harness, cx: &mut TestAppContext, pixels: f32) {
    let position = grid_position(h, cx);
    h.event(
        cx,
        ScrollEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.0), px(pixels))),
            ..Default::default()
        },
    );
    h.draw(cx);
}

fn line_number(text: &str) -> i32 {
    text.strip_prefix("line-")
        .expect("a numbered synthetic line")
        .parse()
        .expect("a line number")
}

fn mouse_down(h: &Harness, cx: &mut TestAppContext, position: gpui_kit::Point<gpui_kit::Pixels>) {
    h.event(
        cx,
        MouseDownEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            modifiers: Modifiers::default(),
            ..Default::default()
        },
    );
}

fn mouse_move(h: &Harness, cx: &mut TestAppContext, position: gpui_kit::Point<gpui_kit::Pixels>) {
    h.event(
        cx,
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        },
    );
}

fn mouse_up(h: &Harness, cx: &mut TestAppContext, position: gpui_kit::Point<gpui_kit::Pixels>) {
    h.event(
        cx,
        MouseUpEvent {
            button: MouseButton::Left,
            position,
            click_count: 1,
            modifiers: Modifiers::default(),
        },
    );
}

#[gpui_kit::test]
fn the_wheel_browses_a_full_finite_history_to_both_boundaries(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, CAPACITY);
    fill(&h, cx, 200);
    let bottom = visible_text(&h, cx);
    assert_eq!(display_offset(&h, cx), 0);
    assert_eq!(history_size(&h, cx), CAPACITY);
    let bottom_top = line_number(&bottom[0]);

    let height = cell_height(&h, cx);
    wheel(&h, cx, 3.0 * height);
    assert_eq!(display_offset(&h, cx), 3);
    let three_back = visible_text(&h, cx);
    assert_eq!(
        line_number(&three_back[0]),
        bottom_top - 3,
        "one line of history per three wheel units"
    );

    wheel(&h, cx, 100.0 * height);
    assert_eq!(
        display_offset(&h, cx),
        history_size(&h, cx),
        "the wheel stops at the oldest line"
    );
    let top = visible_text(&h, cx);
    assert_eq!(line_number(&top[0]), bottom_top - CAPACITY as i32);

    wheel(&h, cx, -100.0 * height);
    assert_eq!(display_offset(&h, cx), 0, "the wheel stops at the bottom");
    assert_eq!(visible_text(&h, cx), bottom);

    wheel(&h, cx, -height);
    assert_eq!(
        display_offset(&h, cx),
        0,
        "wheel-down at the bottom does nothing"
    );
}

#[gpui_kit::test]
fn output_while_scrolled_keeps_the_anchor_until_it_is_evicted(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, CAPACITY);
    fill(&h, cx, 200);
    let height = cell_height(&h, cx);
    wheel(&h, cx, 3.0 * height);
    let anchor = visible_text(&h, cx)[0].clone();

    for index in 200..203 {
        h.output(cx, format!("extra-{index}\r\n").as_bytes());
        assert_eq!(
            visible_text(&h, cx)[0],
            anchor,
            "output before the anchor leaves the viewport must keep it in place"
        );
    }
    assert_eq!(display_offset(&h, cx), CAPACITY);

    h.output(cx, b"extra-203\r\n");
    let after_eviction = visible_text(&h, cx);
    assert_eq!(display_offset(&h, cx), CAPACITY);
    assert_eq!(
        line_number(&after_eviction[0]),
        line_number(&anchor) + 1,
        "once the anchor is evicted the viewport moves one line at a time"
    );
}

#[gpui_kit::test]
fn dragging_the_scrollbar_reaches_the_history_bounds(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 30);
    fill(&h, cx, 100);
    let bottom = visible_text(&h, cx);
    let history = history_size(&h, cx);
    assert_eq!(history, 30);
    let track = h.view.read_with(cx, |view, _| {
        let frame = view.last_frame.as_ref().expect("a prepared frame");
        let metrics = view.geometry.expect("prepared metrics");
        frame.grid().screen_lines as f32 * metrics.cell_height
    });

    let top = gutter_position(&h, cx, 1.0);
    let redraws_before = h.view.read_with(cx, |view, _| view.stats().redraw_requests);
    mouse_down(&h, cx, top);
    let redraws_after = h.view.read_with(cx, |view, _| view.stats().redraw_requests);
    assert!(
        redraws_after > redraws_before,
        "pressing the track asks for the frame that shows the new offset"
    );
    assert_eq!(display_offset(&h, cx), history);
    h.draw(cx);
    assert_eq!(
        line_number(&visible_text(&h, cx)[0]),
        line_number(&bottom[0]) - history as i32,
        "dragging to the top shows the oldest retained line"
    );

    let bottom_gutter = gutter_position(&h, cx, track - 1.0);
    mouse_move(&h, cx, bottom_gutter);
    h.draw(cx);
    assert_eq!(display_offset(&h, cx), 0);
    assert_eq!(visible_text(&h, cx), bottom);
    mouse_up(&h, cx, bottom_gutter);

    let middle_gutter = gutter_position(&h, cx, track / 2.0);
    mouse_down(&h, cx, middle_gutter);
    h.draw(cx);
    let middle = display_offset(&h, cx);
    assert!(
        (middle as i32 - history as i32 / 2).abs() <= 1,
        "the middle of the track selects roughly the middle of the history: {middle}"
    );
    let middle_text = visible_text(&h, cx);
    assert!(middle_text[0].starts_with("line-"));
    assert!(line_number(&middle_text[0]) < line_number(&bottom[0]));
    mouse_up(&h, cx, middle_gutter);
}

#[gpui_kit::test]
fn clearing_the_history_resets_offset_and_notifies_the_frame(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, CAPACITY);
    fill(&h, cx, 200);
    let height = cell_height(&h, cx);
    wheel(&h, cx, 3.0 * height);
    assert_eq!(display_offset(&h, cx), 3);
    let before = generation(&h, cx);

    // ED 2 clears the viewport and ED 3 clears the scrollback, which is what
    // a shell's `clear` emits. The view must follow both in order.
    h.output(cx, b"\x1b[H\x1b[2J\x1b[3J");
    assert_eq!(history_size(&h, cx), 0);
    assert_eq!(display_offset(&h, cx), 0);
    assert!(
        generation(&h, cx) > before,
        "clearing the history invalidates absolute-line state"
    );
    assert!(
        visible_text(&h, cx).iter().all(|line| line.is_empty()),
        "the cleared viewport is blank"
    );

    h.output(cx, b"fresh-0\r\nfresh-1\r\n");
    assert_eq!(history_size(&h, cx), 0);
    let fresh = visible_text(&h, cx);
    assert!(
        fresh.iter().any(|line| line == "fresh-1"),
        "output after the clear is visible: {fresh:?}"
    );
    assert!(
        fresh.iter().all(|line| !line.starts_with("line-")),
        "no cached row from the old generation is published: {fresh:?}"
    );
}

#[gpui_kit::test]
fn clearing_the_viewport_or_alternate_screen_is_not_clearing_main_history(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, CAPACITY);
    fill(&h, cx, 200);
    let before = h.terminal.snapshot();
    assert!(before.history_size() > 0);

    h.output(cx, b"\x1b[2J");
    assert!(
        history_size(&h, cx) > 0,
        "ED 2 blanked the viewport but kept the main history"
    );
    assert!(visible_text(&h, cx).iter().all(|line| line.is_empty()));

    h.output(cx, b"\x1b[?1049h\x1b[H\x1b[2J\x1b[3J");
    assert_eq!(
        history_size(&h, cx),
        0,
        "the alternate screen has no history"
    );
    h.output(cx, b"\x1b[?1049l");
    assert_eq!(
        history_size(&h, cx),
        before.history_size(),
        "leaving the alternate screen restores the untouched main history"
    );
    let height = cell_height(&h, cx);
    wheel(&h, cx, 1_000.0 * height);
    assert!(
        visible_text(&h, cx).iter().any(|line| line == "line-199"),
        "the main history still holds its newest output"
    );
}
