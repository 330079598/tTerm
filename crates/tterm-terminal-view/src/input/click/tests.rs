//! Tests for click counting and for the selection kinds it maps to.
//!
//! The selection itself is Alacritty's, reached through the model, so these
//! tests also prove that the kinds connect to P0-09's selection interface.
//! The click rules are Alacritty's `on_mouse_press` and `on_left_click` in
//! `input/mod.rs`.

use std::time::Duration;
use std::time::Instant;

use tterm_terminal::grid::CellSize;
use tterm_terminal::grid::GridPoint;
use tterm_terminal::grid::GridSize;
use tterm_terminal::selection::SelectionKind;
use tterm_terminal::selection::Side;
use tterm_terminal::terminal::Config;
use tterm_terminal::terminal::Terminal;

use super::CLICK_THRESHOLD;
use super::ClickButton;
use super::ClickCount;
use super::ClickTracker;
use super::selection_kind;

fn terminal(columns: usize, screen_lines: usize) -> Terminal {
    Terminal::new(
        Config::default(),
        GridSize::new(columns, screen_lines),
        CellSize::default(),
    )
}

fn point(line: i32, column: usize) -> GridPoint {
    GridPoint::new(line, column)
}

#[test]
fn clicks_count_within_the_threshold() {
    let mut tracker = ClickTracker::new();
    let start = Instant::now();
    assert_eq!(tracker.press(ClickButton::Left, start), ClickCount::Single);
    assert_eq!(
        tracker.press(ClickButton::Left, start + Duration::from_millis(100)),
        ClickCount::Double
    );
    assert_eq!(
        tracker.press(ClickButton::Left, start + Duration::from_millis(200)),
        ClickCount::Triple
    );
    // A fourth click inside the interval starts a new count, as Alacritty's
    // TripleClick falls through to Click.
    assert_eq!(
        tracker.press(ClickButton::Left, start + Duration::from_millis(300)),
        ClickCount::Single
    );
}

#[test]
fn a_slow_click_starts_over() {
    let mut tracker = ClickTracker::new();
    let start = Instant::now();
    tracker.press(ClickButton::Left, start);
    assert_eq!(
        tracker.press(
            ClickButton::Left,
            start + CLICK_THRESHOLD + Duration::from_millis(1)
        ),
        ClickCount::Single
    );
    // A different button also starts over.
    assert_eq!(
        tracker.press(ClickButton::Right, start + CLICK_THRESHOLD * 2),
        ClickCount::Single
    );
}

#[test]
fn reset_forgets_the_previous_click() {
    let mut tracker = ClickTracker::new();
    let start = Instant::now();
    tracker.press(ClickButton::Left, start);
    tracker.reset();
    assert_eq!(
        tracker.press(ClickButton::Left, start + Duration::from_millis(100)),
        ClickCount::Single
    );
}

#[test]
fn selection_kinds_follow_alacritty() {
    assert_eq!(
        selection_kind(ClickCount::Single, false),
        Some(SelectionKind::Simple)
    );
    assert_eq!(
        selection_kind(ClickCount::Single, true),
        Some(SelectionKind::Block)
    );
    assert_eq!(
        selection_kind(ClickCount::Double, false),
        Some(SelectionKind::Semantic)
    );
    assert_eq!(
        selection_kind(ClickCount::Triple, false),
        Some(SelectionKind::Lines)
    );
    // Control suppresses the word and line selections.
    assert_eq!(selection_kind(ClickCount::Double, true), None);
    assert_eq!(selection_kind(ClickCount::Triple, true), None);
}

#[test]
fn double_click_selects_a_word() {
    let terminal = terminal(20, 5);
    terminal.write(b"hello world");
    terminal.select(
        SelectionKind::Semantic,
        point(0, 6),
        Side::Left,
    );
    assert_eq!(terminal.selection_text().as_deref(), Some("world"));
}

#[test]
fn double_click_selects_a_wide_word() {
    let terminal = terminal(20, 5);
    terminal.write("你好 world".as_bytes());
    terminal.select(SelectionKind::Semantic, point(0, 0), Side::Left);
    assert_eq!(terminal.selection_text().as_deref(), Some("你好"));
}

#[test]
fn double_click_stops_at_a_separator() {
    let terminal = terminal(20, 5);
    terminal.write(b"foo|bar baz");
    terminal.select(SelectionKind::Semantic, point(0, 0), Side::Left);
    assert_eq!(terminal.selection_text().as_deref(), Some("foo"));
}

#[test]
fn double_click_follows_a_wrapped_word() {
    // Five columns force "abcdef" onto two rows.
    let terminal = terminal(5, 4);
    terminal.write(b"abcdef");
    terminal.select(SelectionKind::Semantic, point(0, 0), Side::Left);
    assert_eq!(terminal.selection_text().as_deref(), Some("abcdef"));
}

#[test]
fn triple_click_selects_the_line() {
    let terminal = terminal(20, 4);
    terminal.write(b"first\r\nsecond");
    terminal.select(SelectionKind::Lines, point(0, 1), Side::Left);
    assert_eq!(terminal.selection_text().as_deref(), Some("second"));
}
