//! Overview marks through the real view, scrollbar listeners and frames.
//!
//! The bytes are synthetic OSC 133 reports, but the model, the view and the
//! pointer/wheel listeners are the production ones. Assertions read the frame
//! the view prepared, not a separately constructed snapshot or screen view.

use super::input_tests::Harness;
use super::overview::{self, OverviewSearchMark};
use super::scrollbar::ScrollbarGeometry;
use gpui_kit::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollDelta, ScrollEvent, TestAppContext,
    point, px,
};
use tterm_terminal::command_marks::CommandMarkPosition;

fn commands(count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for index in 0..count {
        bytes.extend_from_slice(format!("\x1b]133;A\x07$ cmd{index}\r\n").as_bytes());
        bytes.extend_from_slice(b"\x1b]133;C\x07");
        bytes.extend_from_slice(format!("out{index}\r\n").as_bytes());
        bytes.extend_from_slice(b"\x1b]133;D;0\x07");
    }
    bytes
}

fn fill(count: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(count * 12);
    for index in 0..count {
        bytes.extend_from_slice(format!("fill-{index:03}\r\n").as_bytes());
    }
    bytes
}

fn marks(h: &Harness, cx: &mut TestAppContext) -> Vec<CommandMarkPosition> {
    h.view.read_with(cx, |view, _| {
        view.last_frame
            .as_ref()
            .expect("a prepared frame")
            .snapshot
            .command_marks()
            .to_vec()
    })
}

fn snapshot(h: &Harness, cx: &mut TestAppContext) -> tterm_terminal::snapshot::Snapshot {
    h.view.read_with(cx, |view, _| {
        view.last_frame
            .as_ref()
            .expect("a prepared frame")
            .snapshot
            .clone()
    })
}

fn cell_height(h: &Harness, cx: &mut TestAppContext) -> f32 {
    h.view.read_with(cx, |view, _| {
        view.geometry.expect("prepared metrics").cell_height
    })
}

fn geometry(
    h: &Harness,
    cx: &mut TestAppContext,
    snapshot: &tterm_terminal::snapshot::Snapshot,
) -> ScrollbarGeometry {
    let lines = snapshot.grid().screen_lines;
    let track = lines as f32 * cell_height(h, cx);
    ScrollbarGeometry::new(
        track,
        lines,
        snapshot.history_size(),
        snapshot.display_offset(),
    )
}

fn layout(
    h: &Harness,
    cx: &mut TestAppContext,
    snapshot: &tterm_terminal::snapshot::Snapshot,
) -> overview::OverviewLayout {
    overview::layout_for_snapshot(&geometry(h, cx, snapshot), snapshot, &[])
}

fn screen_row(snapshot: &tterm_terminal::snapshot::Snapshot, overview_line: i64) -> Option<usize> {
    let line = overview_line - snapshot.history_size() as i64;
    i32::try_from(line)
        .ok()
        .and_then(|line| snapshot.screen_row(line))
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

#[gpui_kit::test]
fn marks_stay_on_their_lines_through_scrollbar_drags_and_appended_output(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    let mut bytes = commands(3);
    bytes.extend_from_slice(&fill(120));
    h.output(cx, &bytes);

    let before_marks = marks(&h, cx);
    assert_eq!(before_marks.len(), 3);
    let before = snapshot(&h, cx);
    assert!(before.history_size() > 0);
    let before_layout = layout(&h, cx, &before);
    assert!(!before_layout.rects.is_empty());
    let before_bottom = before_layout
        .rects
        .iter()
        .map(|rect| rect.y)
        .fold(0.0, f32::max);

    // The oldest prompt is above the bottom viewport.
    assert_eq!(screen_row(&before, before_marks[0].prompt_line), None);

    let track_lines = before.grid().screen_lines;
    let track = geometry(&h, cx, &before).track;
    let top = gutter_position(&h, cx, 1.0);
    mouse_down(&h, cx, top);
    h.draw(cx);
    let at_top = snapshot(&h, cx);
    assert_eq!(at_top.display_offset(), at_top.history_size());
    assert_eq!(marks(&h, cx), before_marks, "a drag does not move a mark");
    assert_eq!(
        screen_row(&at_top, before_marks[0].prompt_line),
        Some(0),
        "the oldest prompt enters the viewport at the top"
    );
    assert_eq!(
        layout(&h, cx, &at_top),
        before_layout,
        "the ruler stays on document lines while the viewport moves"
    );
    mouse_up(&h, cx, top);

    let bottom_y = track_lines as f32 * cell_height(&h, cx) - 1.0;
    let bottom = gutter_position(&h, cx, bottom_y);
    mouse_down(&h, cx, bottom);
    h.draw(cx);
    assert_eq!(snapshot(&h, cx).display_offset(), 0);
    mouse_up(&h, cx, bottom);
    assert_eq!(marks(&h, cx), before_marks);

    // Capacity is not full, so the append adds lines without eviction.
    h.output(cx, &fill(200));
    let after = snapshot(&h, cx);
    assert!(after.history_size() > before.history_size());
    assert!(after.history_size() < 500);
    assert_eq!(marks(&h, cx), before_marks);
    let after_layout = layout(&h, cx, &after);
    let after_bottom = after_layout
        .rects
        .iter()
        .map(|rect| rect.y)
        .fold(0.0, f32::max);
    assert!(
        after_bottom < before_bottom,
        "the marks move up the ruler as the retained buffer grows"
    );
    assert_eq!(track, geometry(&h, cx, &after).track);
}

#[gpui_kit::test]
fn dense_marks_and_the_search_input_are_bounded_and_leave_scrolling_alone(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 1_000);
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_marks().is_empty()),
        "production supplies no fake search results"
    );
    h.output(cx, &commands(300));
    assert_eq!(marks(&h, cx).len(), 300);
    let dense = snapshot(&h, cx);
    assert!(dense.history_size() > 0);
    let rows = geometry(&h, cx, &dense).track.ceil() as usize;
    let painted = h.view.read_with(cx, |view, _| view.stats().overview_marks);
    assert!(painted > 0);
    assert!(
        painted <= (rows * 2) as u64,
        "dense command marks merge into at most two runs per pixel row"
    );

    h.view.update(cx, |view, cx| {
        view.set_search_marks(
            (0..2_000)
                .map(|line| OverviewSearchMark {
                    line,
                    active: line == 1_000,
                })
                .collect(),
            cx,
        );
    });
    h.draw(cx);
    assert_eq!(
        h.view.read_with(cx, |view, _| view.search_marks().len()),
        2_000
    );
    let painted = h.view.read_with(cx, |view, _| view.stats().overview_marks);
    assert!(
        painted <= (rows * 2) as u64,
        "dense search marks merge into at most two runs per pixel row"
    );

    let height = cell_height(&h, cx);
    let offset_before = snapshot(&h, cx).display_offset();
    wheel(&h, cx, 3.0 * height);
    assert!(
        snapshot(&h, cx).display_offset() > offset_before,
        "the wheel still scrolls history with overview marks present"
    );

    h.view
        .update(cx, |view, cx| view.set_search_marks(Vec::new(), cx));
    h.draw(cx);
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_marks().is_empty())
    );
    assert_eq!(marks(&h, cx).len(), 300);
}

#[gpui_kit::test]
fn the_alternate_screen_does_not_paint_main_screen_marks(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    let mut bytes = commands(1);
    bytes.extend_from_slice(&fill(100));
    h.output(cx, &bytes);
    assert!(h.view.read_with(cx, |view, _| view.stats().overview_marks) > 0);

    h.output(cx, b"\x1b[?1049h\x1b[H\x1b[2J");
    assert_eq!(
        h.view.read_with(cx, |view, _| view.stats().overview_marks),
        0,
        "the alternate screen hides the main ruler"
    );
    h.output(cx, b"\x1b[?1049l");
    assert!(
        h.view.read_with(cx, |view, _| view.stats().overview_marks) > 0,
        "returning to the main screen restores its marks"
    );
}
