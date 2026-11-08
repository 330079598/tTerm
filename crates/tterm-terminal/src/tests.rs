//! Behaviour tests for the terminal model.
//!
//! They drive the public API the session layer and the view use: writing
//! session output, taking frames, consuming intents, selecting and searching.
//! The last tests are the crash tests P0-09 requires: a hand-written boundary
//! corpus, an overlong-sequence test, and a seeded random stream of a hundred
//! thousand input groups.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use alacritty_terminal::term::TermMode;

use crate::color::{Color, NamedColor, Rgb};
use crate::event::{Clipboard, Intent};
use crate::grid::{CellSize, GridPoint, GridRange, GridSize};
use crate::mode::Modes;
use crate::search::{Search, SearchDirection};
use crate::selection::{SelectionKind, SelectionRange, Side};
use crate::snapshot::{CursorShape, Snapshot};
use crate::terminal::{ClipboardPolicy, Config, Scroll, Terminal};

const CELL: CellSize = CellSize::new(8, 16);

fn terminal(columns: usize, lines: usize, scrollback: usize) -> Terminal {
    Terminal::new(
        Config {
            scrollback,
            ..Config::default()
        },
        GridSize::new(columns, lines),
        CELL,
    )
}

fn row_text(frame: &Snapshot, line: usize) -> String {
    frame
        .row(line)
        .map(|row| row.cells().iter().map(|cell| cell.ch).collect())
        .unwrap_or_default()
}

fn trimmed_row_text(frame: &Snapshot, line: usize) -> String {
    row_text(frame, line).trim_end().to_string()
}

fn screen_text(frame: &Snapshot) -> Vec<String> {
    (0..frame.line_count())
        .map(|line| row_text(frame, line))
        .collect()
}

fn write_replies(intents: Vec<Intent>) -> Vec<Vec<u8>> {
    intents
        .into_iter()
        .filter_map(|intent| match intent {
            Intent::WriteToPty(bytes) => Some(bytes),
            _ => None,
        })
        .collect()
}

fn damaged_lines(frame: &Snapshot) -> Vec<(usize, usize, usize)> {
    frame
        .damage()
        .rows()
        .iter()
        .map(|line| (line.line, line.left, line.right))
        .collect()
}

// ---------------------------------------------------------------------------
// Writing output and reading it back
// ---------------------------------------------------------------------------

#[test]
fn first_frame_contains_the_visible_grid_and_its_defaults() {
    let terminal = terminal(20, 5, 0);
    let frame = terminal.snapshot();

    assert_eq!(frame.version(), 1);
    assert_eq!(frame.grid(), GridSize::new(20, 5));
    assert_eq!(frame.cell_size(), CELL);
    assert_eq!(frame.line_count(), 5);
    assert_eq!(frame.cell_count(), 20 * 5);
    assert_eq!(frame.history_size(), 0);
    assert_eq!(frame.display_offset(), 0);
    assert!(!frame.is_scrolled_back());
    assert!(frame.damage().is_full(), "a new terminal is fully damaged");
    assert_eq!(frame.cursor().point, GridPoint::new(0, 0));
    assert_eq!(frame.cursor().shape, CursorShape::Block);
    assert!(frame.cursor().is_visible());
    assert!(frame.modes().contains(Modes::SHOW_CURSOR));
    assert!(frame.modes().contains(Modes::LINE_WRAP));
    assert!(!frame.bottom_line_occupied());
}

#[test]
fn mode_sets_combine_for_groups() {
    let combined = Modes::APP_CURSOR.union(Modes::APP_KEYPAD);
    assert!(combined.contains(Modes::APP_CURSOR));
    assert!(combined.contains(Modes::APP_KEYPAD));
    assert_eq!(
        combined.bits(),
        Modes::APP_CURSOR.bits() | Modes::APP_KEYPAD.bits()
    );
}

#[test]
fn mouse_motion_modes_map_to_alacritty_flags() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"\x1b[?1002h");
    let frame = terminal.snapshot();
    assert!(frame.modes().contains(Modes::MOUSE_DRAG));
    assert!(!frame.modes().contains(Modes::MOUSE_MOTION));

    terminal.write(b"\x1b[?1003h");
    let frame = terminal.snapshot();
    assert!(frame.modes().contains(Modes::MOUSE_MOTION));
    assert!(!frame.modes().contains(Modes::MOUSE_DRAG));
}

#[test]
fn writes_text_and_places_the_cursor() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"hello");
    let frame = terminal.snapshot();

    assert_eq!(trimmed_row_text(&frame, 0), "hello");
    assert_eq!(frame.cursor().point, GridPoint::new(0, 5));
    assert_eq!(frame.cursor().cell.as_ref().map(|cell| cell.ch), Some(' '));
}

#[test]
fn a_utf8_sequence_split_across_writes_survives() {
    let terminal = terminal(10, 3, 0);
    // "中" is E4 B8 AD; the session reader may hand it over in three pieces.
    terminal.write(b"\xe4");
    terminal.write(b"\xb8");
    terminal.write(b"\xad");
    let frame = terminal.snapshot();

    assert_eq!(
        frame.row(0).and_then(|row| row.cell(0)).map(|cell| cell.ch),
        Some('中')
    );
    assert!(
        frame
            .row(0)
            .is_some_and(|row| row.cell(0).is_some_and(|cell| cell.attributes.wide))
    );
    assert!(
        frame
            .row(0)
            .is_some_and(|row| row.cell(1).is_some_and(|cell| cell.attributes.wide_spacer))
    );
}

#[test]
fn invalid_utf8_becomes_replacement_characters_instead_of_panicking() {
    let terminal = terminal(10, 3, 0);
    terminal.write(b"a\xff\xfe\xef\xbcb");
    let frame = terminal.snapshot();

    let text = trimmed_row_text(&frame, 0);
    assert!(text.starts_with('a'), "{text:?}");
    assert!(text.ends_with('b'), "{text:?}");
    assert!(
        text.contains('\u{fffd}'),
        "invalid bytes become replacement characters: {text:?}"
    );
}

#[test]
fn combining_marks_stay_on_their_cell() {
    let terminal = terminal(10, 3, 0);
    terminal.write("e\u{0301}x".as_bytes());
    let frame = terminal.snapshot();

    let first = frame
        .row(0)
        .and_then(|row| row.cell(0))
        .expect("first cell");
    assert_eq!(first.ch, 'e');
    assert_eq!(first.zerowidth.as_deref(), Some(&['\u{0301}'][..]));
    assert_eq!(
        frame.row(0).and_then(|row| row.cell(1)).map(|cell| cell.ch),
        Some('x')
    );
}

#[test]
fn colors_keep_the_semantic_form_the_application_asked_for() {
    let terminal = terminal(40, 3, 0);
    terminal.write(b"\x1b[31mred\x1b[0m \x1b[38;5;200mpal\x1b[0m \x1b[38;2;1;2;3mtrue\r\n");
    let frame = terminal.snapshot();
    let row = frame.row(0).expect("first row");

    assert_eq!(
        row.cell(0).map(|cell| cell.foreground),
        Some(Color::Named(NamedColor::Red))
    );
    assert_eq!(row.cell(0).map(|cell| cell.ch), Some('r'));
    assert_eq!(
        row.cell(3).map(|cell| cell.foreground),
        Some(Color::Named(NamedColor::Foreground))
    );
    assert_eq!(
        row.cell(4).map(|cell| cell.foreground),
        Some(Color::Indexed(200))
    );
    assert_eq!(row.cell(4).map(|cell| cell.ch), Some('p'));
    assert_eq!(
        row.cell(8).map(|cell| cell.foreground),
        Some(Color::Spec(Rgb { r: 1, g: 2, b: 3 }))
    );
    assert_eq!(row.cell(8).map(|cell| cell.ch), Some('t'));
}

#[test]
fn attributes_and_hyperlinks_are_copied() {
    let terminal = terminal(40, 3, 0);
    terminal.write(b"\x1b[1;4;58;5;9mstyle\x1b[0m");
    terminal.write(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
    let frame = terminal.snapshot();
    let row = frame.row(0).expect("first row");

    let styled = row.cell(0).expect("styled cell");
    assert!(styled.attributes.bold);
    assert_eq!(styled.attributes.underline, crate::cell::Underline::Single);
    assert_eq!(styled.underline_color, Some(Color::Indexed(9)));

    let linked = row.cell(5).expect("linked cell");
    let link = linked.hyperlink.as_ref().expect("hyperlink");
    assert_eq!(link.uri(), "https://example.com");
    assert!(
        link.id().ends_with("_alacritty"),
        "generated id: {}",
        link.id()
    );
}

#[test]
fn state_stays_consistent_after_an_alternate_screen_switch() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"main");
    let before = terminal.snapshot();
    assert_eq!(trimmed_row_text(&before, 0), "main");

    terminal.write(b"\x1b[?1049h");
    let alternate = terminal.snapshot();
    assert!(alternate.modes().contains(Modes::ALT_SCREEN));
    assert!(
        alternate.damage().is_full(),
        "switching screens changes everything"
    );
    assert_eq!(trimmed_row_text(&alternate, 0), "");

    terminal.write(b"\x1b[?1049l");
    let primary = terminal.snapshot();
    assert!(!primary.modes().contains(Modes::ALT_SCREEN));
    assert_eq!(trimmed_row_text(&primary, 0), "main");
}

// ---------------------------------------------------------------------------
// Damage tracking
// ---------------------------------------------------------------------------

#[test]
fn output_that_scrolls_the_screen_is_fully_damaged() {
    let terminal = terminal(20, 5, 100);
    terminal.snapshot();

    // Put the cursor on the last line and let one line feed rotate the screen.
    terminal.write(b"\x1b[5;1H\r\n");
    let scrolled = terminal.snapshot();
    assert!(
        scrolled.damage().is_full(),
        "each screen scroll damages everything"
    );

    // A flood of lines scrolls on almost every byte, so again everything.
    terminal.write(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\n");
    assert!(terminal.snapshot().damage().is_full());
}

#[test]
fn output_while_scrolled_back_is_fully_damaged() {
    let terminal = terminal(20, 4, 100);
    for index in 0..10 {
        terminal.write(format!("line {index}\r\n").as_bytes());
    }
    terminal.scroll(Scroll::Delta(3));
    let reading = terminal.snapshot();
    assert_eq!(reading.display_offset(), 3);
    assert!(!reading.is_scrolled_to_top());

    terminal.write(b"\r\n");
    let after = terminal.snapshot();

    assert!(
        after.damage().is_full(),
        "the viewport moved, so no row keeps its content"
    );
    assert_eq!(
        after.display_offset(),
        4,
        "the display offset grows so the user keeps reading the same history"
    );
    assert!(after.is_scrolled_back());
}

#[test]
fn scrollback_eviction_advances_generation_without_approximate_counts() {
    let terminal = terminal(20, 4, 5);
    terminal.write(b"L00\r\nL01\r\nL02\r\nL03\r\n");
    let before = terminal.snapshot();
    let before_generation = before.scrollback_generation();

    terminal.scroll(Scroll::Delta(2));
    terminal.write(b"L04\r\nL05\r\nL06\r\nL07\r\nL08\r\nL09\r\n");
    let first_eviction = terminal.snapshot();
    assert!(first_eviction.scrollback_generation() > before_generation);
    assert_eq!(trimmed_row_text(&first_eviction, 0), "L02");

    terminal.write(b"L10\r\nL11\r\nL12\r\n");
    let second_eviction = terminal.snapshot();
    assert!(second_eviction.scrollback_generation() > first_eviction.scrollback_generation());
    assert_eq!(trimmed_row_text(&second_eviction, 0), "L05");
}

#[test]
fn long_wrapped_line_evicts_history_without_a_newline() {
    let terminal = terminal(4, 2, 2);
    terminal.write(b"old-0\r\nold-1\r\n");
    let before = terminal.snapshot();
    terminal.scroll(Scroll::Top);
    terminal.snapshot();

    terminal.write(b"abcdefghijklmno");
    let after = terminal.snapshot();
    assert!(after.scrollback_generation() > before.scrollback_generation());
    assert!(after.history_size() <= 2);
    assert!(
        after
            .rows()
            .iter()
            .any(|row| { row.cells().iter().any(|cell| cell.ch != ' ') })
    );
}

#[test]
fn evicting_then_entering_the_alternate_screen_in_one_write_advances_the_generation() {
    let terminal = terminal(20, 3, 2);
    terminal.write(b"old-0\r\nold-1\r\nold-2\r\nold-3\r\nold-4\r\n");
    let before = terminal.snapshot();
    terminal.write(b"new-0\r\nnew-1\r\n\x1b[?1049h");
    let after = terminal.snapshot();
    assert!(after.modes().contains(crate::mode::Modes::ALT_SCREEN));
    assert!(after.scrollback_generation() > before.scrollback_generation());
}

#[test]
fn csi_scroll_up_and_index_evict_history() {
    for sequence in [b"\x1b[3S".as_slice(), b"\x1bD".as_slice()] {
        let terminal = terminal(20, 3, 2);
        terminal.write(b"old-0\r\nold-1\r\nold-2\r\n");
        let before = terminal.snapshot();
        terminal.write(sequence);
        let after = terminal.snapshot();
        assert!(
            after.scrollback_generation() > before.scrollback_generation(),
            "sequence {sequence:?} must advance the generation when history is full"
        );
    }
}

#[test]
fn middle_linefeed_and_partial_scroll_region_do_not_mix_view_content() {
    let terminal = terminal(12, 5, 4);
    terminal.write(b"seed-0\r\nseed-1\r\nseed-2\r\nseed-3\r\n");
    let before = terminal.snapshot();

    terminal.write(b"\x1b[3;1H\r\n");
    let middle = terminal.snapshot();
    assert!(middle.rows().iter().all(|row| !row.cells().is_empty()));

    terminal.write(b"\x1b[2;4r\x1b[4;1H\r\n\x1b[r");
    let region = terminal.snapshot();
    assert!(region.rows().iter().all(|row| !row.cells().is_empty()));
    assert!(region.scrollback_generation() >= before.scrollback_generation());
}

#[test]
fn clearing_history_then_output_starts_a_consistent_generation() {
    let terminal = terminal(12, 4, 3);
    terminal.write(b"old-0\r\nold-1\r\nold-2\r\nold-3\r\n");
    terminal.scroll(Scroll::Top);
    terminal.snapshot();
    terminal.write(b"\x1b[3J");
    let cleared = terminal.snapshot();
    assert_eq!(cleared.history_size(), 0);
    terminal.write(b"fresh\r\n");
    let after = terminal.snapshot();
    assert!(after.history_size() <= 1);
    assert!(after.rows().iter().all(|row| !row.cells().is_empty()));
}

#[test]
fn scrolling_back_and_returning_keeps_unsubmitted_bottom_input() {
    let terminal = terminal(20, 4, 20);
    terminal.write(b"one\r\ntwo\r\nthree\r\nfour\r\n");
    terminal.write(b"draft");
    terminal.scroll(Scroll::Top);
    let history = terminal.snapshot();
    assert!(history.is_scrolled_back());
    terminal.scroll(Scroll::Bottom);
    let bottom = terminal.snapshot();
    assert!(
        screen_text(&bottom)
            .iter()
            .any(|line| line.contains("draft"))
    );
}

#[test]
fn rapid_output_while_scrolled_keeps_complete_frames() {
    let terminal = terminal(20, 4, 5);
    for index in 0..20 {
        terminal.write(format!("seed-{index}\r\n").as_bytes());
    }
    terminal.scroll(Scroll::Delta(2));
    for index in 0..20 {
        terminal.write(format!("burst-{index}\r\n").as_bytes());
        let frame = terminal.snapshot();
        assert_eq!(frame.rows().len(), 4);
        assert!(frame.rows().iter().all(|row| !row.cells().is_empty()));
    }
}

#[test]
fn an_evicted_anchor_moves_like_xterm_after_the_offset_limit() {
    let terminal = terminal(20, 4, 5);
    for index in 0..20 {
        terminal.write(format!("line-{index}\r\n").as_bytes());
    }
    terminal.scroll(Scroll::Delta(2));
    let mut first_rows = Vec::new();
    for index in 20..28 {
        terminal.write(format!("line-{index}\r\n").as_bytes());
        first_rows.push(trimmed_row_text(&terminal.snapshot(), 0));
    }
    assert_eq!(&first_rows[..3], ["line-15", "line-15", "line-15"]);
    assert!(first_rows[3..].windows(2).all(|rows| rows[0] != rows[1]));
}

#[test]
fn in_place_updates_damage_only_their_rows() {
    let terminal = terminal(20, 5, 0);
    terminal.snapshot();

    // Move away from the cursor's row and rewrite a single cell there.
    terminal.write(b"\x1b[4;6Hz");
    let frame = terminal.snapshot();

    assert!(
        !frame.damage().is_full(),
        "no scrolling happened, so damage is per row"
    );
    let damaged = damaged_lines(&frame);
    assert!(
        damaged
            .iter()
            .any(|(line, left, right)| *line == 3 && *left <= 5 && *right >= 5),
        "the edited row must be damaged: {damaged:?}"
    );
    assert!(
        !damaged.iter().any(|(line, _, _)| *line == 1 || *line == 2),
        "rows that were not touched must not be damaged: {damaged:?}"
    );
}

#[test]
fn damage_is_consumed_by_a_snapshot_and_reset() {
    let terminal = terminal(20, 5, 0);
    terminal.snapshot();

    terminal.write(b"\x1b[1;1Hxyz");
    let edited = terminal.snapshot();
    assert!(
        damaged_lines(&edited)
            .iter()
            .any(|(line, left, right)| *line == 0 && *left == 0 && *right >= 2),
        "the editing frame must report the written columns: {:?}",
        damaged_lines(&edited)
    );

    // Nothing changed since; only the cursor cell itself may be damaged.
    let idle = terminal.snapshot();
    assert_eq!(
        damaged_lines(&idle),
        vec![(0, 3, 3)],
        "the previous frame's columns must not come back"
    );
}

#[test]
fn the_cursor_row_is_damaged_in_every_frame() {
    let terminal = terminal(20, 5, 0);
    terminal.snapshot();

    terminal.write(b"\x1b[3;1H");
    let moved = terminal.snapshot();
    assert!(
        damaged_lines(&moved).iter().any(|(line, _, _)| *line == 2),
        "the row the cursor moved to must be reported: {:?}",
        damaged_lines(&moved)
    );

    let idle = terminal.snapshot();
    assert!(
        damaged_lines(&idle).iter().any(|(line, _, _)| *line == 2),
        "the cursor row is part of every damaged frame, even without output"
    );
}

// ---------------------------------------------------------------------------
// Snapshot cost: visible lines, never the scrollback
// ---------------------------------------------------------------------------

#[test]
fn snapshot_copy_volume_depends_on_visible_lines_not_scrollback() {
    const COLUMNS: usize = 40;
    const LINES: usize = 24;
    const FED_LINES: usize = 12_000;

    let mut output = Vec::with_capacity(FED_LINES * 13);
    for index in 0..FED_LINES {
        output.extend_from_slice(format!("line {index:05}\r\n").as_bytes());
    }

    let small = terminal(COLUMNS, LINES, 1_000);
    let large = terminal(COLUMNS, LINES, 100_000);
    small.write(&output);
    large.write(&output);

    let small_frame = small.snapshot();
    let large_frame = large.snapshot();

    assert_eq!(small.scrollback(), 1_000);
    assert_eq!(large.scrollback(), 100_000);
    assert_eq!(
        small_frame.history_size(),
        1_000,
        "the small session is at its cap"
    );
    assert!(
        large_frame.history_size() > 10_000,
        "the large session holds far more history: {}",
        large_frame.history_size()
    );

    // The evaluation point: the same 24 visible rows cost the same copy
    // volume, and hold the same content, although one session keeps ten times
    // as much history as the other and a hundred times the capacity.
    assert_eq!(small_frame.cell_count(), LINES * COLUMNS);
    assert_eq!(large_frame.cell_count(), LINES * COLUMNS);
    assert_eq!(small_frame.cell_count(), large_frame.cell_count());
    assert_eq!(small_frame.line_count(), LINES);
    assert_eq!(large_frame.line_count(), LINES);
    assert_eq!(screen_text(&small_frame), screen_text(&large_frame));
    eprintln!(
        "snapshot copy: {} history lines held with a 1000-line cap vs {} with a 100000-line cap; \
         {} cells copied from each",
        small_frame.history_size(),
        large_frame.history_size(),
        small_frame.cell_count(),
    );
}

#[test]
fn a_scrolled_back_frame_still_copies_only_the_visible_rows() {
    const COLUMNS: usize = 40;
    const LINES: usize = 24;

    let terminal = terminal(COLUMNS, LINES, 100_000);
    for index in 0..12_000 {
        terminal.write(format!("line {index:05}\r\n").as_bytes());
    }
    let bottom = terminal.snapshot();
    assert!(!bottom.is_scrolled_back());
    assert!(bottom.is_scrolled_to_bottom());

    terminal.scroll(Scroll::Top);
    let top = terminal.snapshot();

    assert_eq!(top.display_offset(), top.history_size());
    assert!(top.is_scrolled_back());
    assert_eq!(top.cell_count(), LINES * COLUMNS);
    assert_eq!(top.line_count(), LINES);
    assert_ne!(screen_text(&top), screen_text(&bottom));
    assert_eq!(
        top.screen_row(0),
        None,
        "line zero is below the viewport when scrolled back"
    );
    assert_eq!(top.screen_row(-(top.display_offset() as i32)), Some(0));
    assert!(
        top.cursor().cell.is_none(),
        "the cursor is below the viewport while the user reads history"
    );
}

#[test]
fn screen_rows_follow_the_display_offset() {
    let terminal = terminal(10, 4, 100);
    for index in 0..10 {
        terminal.write(format!("line {index}\r\n").as_bytes());
    }
    terminal.scroll(Scroll::Delta(3));
    let frame = terminal.snapshot();

    assert_eq!(frame.display_offset(), 3);
    assert_eq!(frame.screen_row(-3), Some(0));
    assert_eq!(frame.screen_row(0), Some(3));
    assert_eq!(frame.screen_row(-4), None, "above the oldest visible line");
    assert_eq!(frame.screen_row(1), None, "below the viewport");

    terminal.scroll(Scroll::PageUp);
    assert_eq!(terminal.snapshot().display_offset(), 7);
    terminal.scroll(Scroll::PageDown);
    assert_eq!(terminal.snapshot().display_offset(), 3);
    terminal.scroll(Scroll::Top);
    assert!(terminal.snapshot().is_scrolled_to_top());
    terminal.scroll(Scroll::Bottom);
    assert!(terminal.snapshot().is_scrolled_to_bottom());
}

// ---------------------------------------------------------------------------
// Scrollback and resize
// ---------------------------------------------------------------------------

#[test]
fn scrollback_capacity_bounds_the_history() {
    let terminal = terminal(20, 3, 5);
    for index in 0..50 {
        terminal.write(format!("line {index}\r\n").as_bytes());
    }
    let frame = terminal.snapshot();

    assert_eq!(terminal.scrollback(), 5);
    assert_eq!(frame.history_size(), 5);
    assert_eq!(trimmed_row_text(&frame, 1), "line 49");
}

#[test]
fn changing_the_scrollback_capacity_drops_the_oldest_lines() {
    let terminal = terminal(20, 3, 50);
    for index in 0..50 {
        terminal.write(format!("line {index}\r\n").as_bytes());
    }
    terminal.snapshot();
    assert_eq!(terminal.snapshot().history_size(), 48);

    terminal.set_scrollback(2);
    let shrunk = terminal.snapshot();

    assert_eq!(terminal.scrollback(), 2);
    assert_eq!(shrunk.history_size(), 2);
    assert!(
        shrunk.damage().is_full(),
        "dropping history changes the whole screen"
    );
    assert_eq!(trimmed_row_text(&shrunk, 1), "line 49");
}

#[test]
fn resize_replaces_the_grid_and_damages_everything() {
    let terminal = terminal(10, 3, 0);
    terminal.write(b"hello");
    terminal.snapshot();

    terminal.resize(GridSize::new(20, 6), CellSize::new(9, 18));
    let frame = terminal.snapshot();

    assert_eq!(frame.grid(), GridSize::new(20, 6));
    assert_eq!(terminal.grid_size(), GridSize::new(20, 6));
    assert_eq!(frame.cell_size(), CellSize::new(9, 18));
    assert_eq!(frame.line_count(), 6);
    assert_eq!(frame.cell_count(), 20 * 6);
    assert!(
        frame.damage().is_full(),
        "a resize changes the row of every cell"
    );
    assert_eq!(trimmed_row_text(&frame, 0), "hello");
}

#[test]
fn a_zero_sized_resize_is_clamped_instead_of_panicking() {
    let terminal = terminal(10, 3, 0);
    terminal.resize(GridSize::new(0, 0), CellSize::new(0, 0));
    let frame = terminal.snapshot();

    assert_eq!(frame.grid(), GridSize::new(2, 1));
    assert_eq!(frame.line_count(), 1);
    assert_eq!(frame.cell_count(), 2);
}

#[test]
fn a_one_column_grid_is_raised_to_two() {
    // Alacritty 0.26.0's reflow did not finish when the crash test resized a
    // terminal with wide characters in its history to one column, so the model
    // never represents such a grid. See the report's deviation section.
    assert_eq!(GridSize::new(1, 5), GridSize::new(2, 5));

    let terminal = terminal(1, 5, 0);
    terminal.write("中中中".as_bytes());
    let frame = terminal.snapshot();
    assert_eq!(frame.grid().columns, 2);
    assert_eq!(frame.cell_count(), 2 * 5);
}

#[test]
fn a_size_built_from_its_fields_is_clamped_too() {
    // The fields of `GridSize` are public, so a view can build a one-column,
    // zero-row size without `GridSize::new`. The model must clamp it anyway,
    // on creation and on resize, before Alacritty sees it.
    let narrow = GridSize {
        columns: 1,
        screen_lines: 0,
    };
    let terminal = Terminal::new(Config::default(), narrow, CELL);
    terminal.write("中中中".as_bytes());
    assert_eq!(terminal.snapshot().grid(), GridSize::new(2, 1));

    terminal.resize(
        GridSize {
            columns: 20,
            screen_lines: 4,
        },
        CELL,
    );
    terminal.write("中中中".as_bytes());
    terminal.resize(narrow, CELL);
    let frame = terminal.snapshot();
    assert_eq!(frame.grid(), GridSize::new(2, 1));
    assert_eq!(frame.cell_count(), 2);
}

#[test]
fn the_bottom_line_flag_tracks_content_in_the_last_row() {
    let terminal = terminal(10, 3, 0);
    terminal.write(b"a");
    assert!(!terminal.snapshot().bottom_line_occupied());

    terminal.write(b"\x1b[3;1Hb");
    assert!(terminal.snapshot().bottom_line_occupied());
}

// ---------------------------------------------------------------------------
// Intents
// ---------------------------------------------------------------------------

#[test]
fn title_and_bell_arrive_as_intents() {
    let subject = terminal(20, 5, 0);

    subject.write(b"\x1b]0;a title\x07");
    let intents = subject.drain_intents();
    assert!(matches!(&intents[..], [Intent::Title(title)] if title == "a title"));

    subject.write(b"\x07");
    let intents = subject.drain_intents();
    assert!(matches!(&intents[..], [Intent::Bell]));

    subject.set_scrollback(500);
    let intents = subject.drain_intents();
    assert!(
        matches!(&intents[..], [Intent::Title(title)] if title == "a title"),
        "changing options re-announces the current title: {intents:?}"
    );

    let fresh = terminal(20, 5, 0);
    fresh.set_scrollback(500);
    assert!(
        matches!(&fresh.drain_intents()[..], [Intent::TitleReset]),
        "a session that never set a title reports the default"
    );
}

#[test]
fn device_queries_are_answered_as_pty_writes() {
    let terminal = terminal(20, 5, 0);

    terminal.write(b"\x1b[c");
    assert_eq!(
        write_replies(terminal.drain_intents()),
        vec![b"\x1b[?6c".to_vec()]
    );

    terminal.write(b"\x1b[6n");
    assert_eq!(
        write_replies(terminal.drain_intents()),
        vec![b"\x1b[1;1R".to_vec()]
    );

    terminal.write(b"\x1b[5n");
    assert_eq!(
        write_replies(terminal.drain_intents()),
        vec![b"\x1b[0n".to_vec()]
    );

    terminal.write(b"\x1b[>c");
    let replies = write_replies(terminal.drain_intents());
    assert_eq!(replies.len(), 1);
    assert!(replies[0].starts_with(b"\x1b[>0;"));
    assert!(replies[0].ends_with(b"c"));

    assert!(
        terminal.drain_intents().is_empty(),
        "a drained queue stays empty until the next event"
    );
}

#[test]
fn clipboard_stores_are_intents_and_reads_are_denied_by_default() {
    let terminal = terminal(20, 5, 0);

    terminal.write(b"\x1b]52;c;aGVsbG8=\x07");
    let intents = terminal.drain_intents();
    assert!(matches!(
        &intents[..],
        [Intent::ClipboardStore { clipboard: Clipboard::Clipboard, text }] if text == "hello"
    ));

    // Reading is a paste the user did not ask for, so the default policy drops
    // the request instead of producing an intent.
    terminal.write(b"\x1b]52;c;?\x07");
    assert!(terminal.drain_intents().is_empty());
}

#[test]
fn clipboard_reads_can_be_answered_when_the_caller_allows_them() {
    let terminal = Terminal::new(
        Config {
            clipboard: ClipboardPolicy::CopyAndPaste,
            ..Config::default()
        },
        GridSize::new(20, 5),
        CELL,
    );

    terminal.write(b"\x1b]52;c;?\x07");
    let intents = terminal.drain_intents();
    let [Intent::ClipboardRead { clipboard, format }] = &intents[..] else {
        panic!("expected one clipboard read intent, got {intents:?}");
    };
    assert_eq!(*clipboard, Clipboard::Clipboard);
    assert_eq!(format.format("hi"), b"\x1b]52;c;aGk=\x07".to_vec());
}

#[test]
fn the_clipboard_policy_applies_per_direction() {
    let disabled = Terminal::new(
        Config {
            clipboard: ClipboardPolicy::Disabled,
            ..Config::default()
        },
        GridSize::new(20, 5),
        CELL,
    );
    disabled.write(b"\x1b]52;c;aGVsbG8=\x07");
    disabled.write(b"\x1b]52;c;?\x07");
    assert!(
        disabled.drain_intents().is_empty(),
        "a disabled clipboard ignores both directions"
    );

    let paste_only = Terminal::new(
        Config {
            clipboard: ClipboardPolicy::PasteOnly,
            ..Config::default()
        },
        GridSize::new(20, 5),
        CELL,
    );
    paste_only.write(b"\x1b]52;c;aGVsbG8=\x07");
    assert!(
        paste_only.drain_intents().is_empty(),
        "storing is not allowed"
    );
    paste_only.write(b"\x1b]52;c;?\x07");
    assert!(matches!(
        &paste_only.drain_intents()[..],
        [Intent::ClipboardRead { .. }]
    ));
}

#[test]
fn color_queries_are_answered_from_the_palette_by_the_caller() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"\x1b]11;?\x07");

    let intents = terminal.drain_intents();
    let [Intent::ColorRequest { index, format }] = &intents[..] else {
        panic!("expected one color request, got {intents:?}");
    };
    // 257 is the background slot of the 269-entry palette Alacritty indexes
    // (256 foreground, 257 background, 258 cursor).
    assert_eq!(*index, 257);
    assert_eq!(
        format.format(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        }),
        b"\x1b]11;rgb:1212/3434/5656\x07".to_vec()
    );
}

#[test]
fn window_size_queries_are_answered_from_the_snapshot() {
    let terminal = terminal(10, 3, 0);
    terminal.write(b"\x1b[14t");

    let intents = terminal.drain_intents();
    let [Intent::WindowSizeRequest { format }] = &intents[..] else {
        panic!("expected one window size request, got {intents:?}");
    };
    let frame = terminal.snapshot();
    assert_eq!(
        format.format(frame.window_size()),
        b"\x1b[4;48;80t".to_vec()
    );
}

#[test]
fn cursor_style_changes_report_blinking_and_shape() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"\x1b[5 q");

    let intents = terminal.drain_intents();
    assert!(matches!(&intents[..], [Intent::CursorBlinkingChanged]));
    let frame = terminal.snapshot();
    assert_eq!(frame.cursor().shape, CursorShape::Beam);
    assert!(frame.cursor().blinking);
}

#[test]
fn the_kitty_keyboard_protocol_is_opt_in() {
    let off = terminal(20, 5, 0);
    off.write(b"\x1b[>1u");
    assert!(
        !off.snapshot()
            .modes()
            .contains(Modes::DISAMBIGUATE_ESC_CODES)
    );
    assert!(write_replies(off.drain_intents()).is_empty());

    let on = Terminal::new(
        Config {
            kitty_keyboard: true,
            ..Config::default()
        },
        GridSize::new(20, 5),
        CELL,
    );
    on.write(b"\x1b[>1u");
    assert!(
        on.snapshot()
            .modes()
            .contains(Modes::DISAMBIGUATE_ESC_CODES)
    );

    on.write(b"\x1b[?u");
    assert_eq!(
        write_replies(on.drain_intents()),
        vec![b"\x1b[?1u".to_vec()]
    );
}

#[test]
fn modes_follow_the_output() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"\x1b[?1h\x1b[?2004h\x1b[?1002h\x1b[4h");
    let frame = terminal.snapshot();

    assert!(frame.modes().contains(Modes::APP_CURSOR));
    assert!(frame.modes().contains(Modes::BRACKETED_PASTE));
    assert!(frame.modes().contains(Modes::MOUSE_DRAG));
    assert!(frame.modes().intersects(Modes::MOUSE_MODE));
    assert!(frame.modes().contains(Modes::INSERT));

    terminal.write(b"\x1b[?1003h");
    assert!(terminal.snapshot().modes().contains(Modes::MOUSE_MOTION));
}

#[test]
fn mode_bits_match_the_pinned_alacritty() {
    let pairs = [
        (Modes::SHOW_CURSOR, TermMode::SHOW_CURSOR),
        (Modes::APP_CURSOR, TermMode::APP_CURSOR),
        (Modes::APP_KEYPAD, TermMode::APP_KEYPAD),
        (Modes::MOUSE_REPORT_CLICK, TermMode::MOUSE_REPORT_CLICK),
        (Modes::BRACKETED_PASTE, TermMode::BRACKETED_PASTE),
        (Modes::SGR_MOUSE, TermMode::SGR_MOUSE),
        (Modes::MOUSE_MOTION, TermMode::MOUSE_MOTION),
        (Modes::LINE_WRAP, TermMode::LINE_WRAP),
        (Modes::LINE_FEED_NEW_LINE, TermMode::LINE_FEED_NEW_LINE),
        (Modes::ORIGIN, TermMode::ORIGIN),
        (Modes::INSERT, TermMode::INSERT),
        (Modes::FOCUS_IN_OUT, TermMode::FOCUS_IN_OUT),
        (Modes::ALT_SCREEN, TermMode::ALT_SCREEN),
        (Modes::MOUSE_DRAG, TermMode::MOUSE_DRAG),
        (Modes::UTF8_MOUSE, TermMode::UTF8_MOUSE),
        (Modes::ALTERNATE_SCROLL, TermMode::ALTERNATE_SCROLL),
        (Modes::VI, TermMode::VI),
        (Modes::URGENCY_HINTS, TermMode::URGENCY_HINTS),
        (
            Modes::DISAMBIGUATE_ESC_CODES,
            TermMode::DISAMBIGUATE_ESC_CODES,
        ),
        (Modes::REPORT_EVENT_TYPES, TermMode::REPORT_EVENT_TYPES),
        (
            Modes::REPORT_ALTERNATE_KEYS,
            TermMode::REPORT_ALTERNATE_KEYS,
        ),
        (
            Modes::REPORT_ALL_KEYS_AS_ESC,
            TermMode::REPORT_ALL_KEYS_AS_ESC,
        ),
        (
            Modes::REPORT_ASSOCIATED_TEXT,
            TermMode::REPORT_ASSOCIATED_TEXT,
        ),
        (Modes::MOUSE_MODE, TermMode::MOUSE_MODE),
    ];
    for (ours, theirs) in pairs {
        assert_eq!(
            ours.bits(),
            theirs.bits(),
            "{ours:?} does not match Alacritty"
        );
    }
}

// ---------------------------------------------------------------------------
// Selection and search
// ---------------------------------------------------------------------------

#[test]
fn selecting_a_word_reports_the_range_and_the_text() {
    let terminal = terminal(20, 5, 0);
    terminal.write(b"hello\r\nworld");
    terminal.snapshot();

    terminal.select(SelectionKind::Simple, GridPoint::new(0, 0), Side::Left);
    terminal.extend_selection(GridPoint::new(0, 4), Side::Right);
    let frame = terminal.snapshot();

    assert_eq!(
        frame.selection(),
        Some(SelectionRange {
            start: GridPoint::new(0, 0),
            end: GridPoint::new(0, 4),
            block: false,
        })
    );
    assert_eq!(terminal.selection_text(), Some("hello".to_string()));

    terminal.clear_selection();
    assert_eq!(terminal.snapshot().selection(), None);
    assert_eq!(terminal.selection_text(), None);
}

#[test]
fn extending_without_a_selection_does_nothing() {
    let terminal = terminal(20, 5, 0);
    terminal.extend_selection(GridPoint::new(0, 3), Side::Right);

    assert_eq!(terminal.snapshot().selection(), None);
    assert_eq!(terminal.selection_text(), None);
}

#[test]
fn a_selection_range_knows_which_cells_it_covers() {
    let reading_order = SelectionRange {
        start: GridPoint::new(0, 4),
        end: GridPoint::new(2, 3),
        block: false,
    };
    assert!(!reading_order.contains(GridPoint::new(0, 3)));
    assert!(reading_order.contains(GridPoint::new(0, 4)));
    assert!(
        reading_order.contains(GridPoint::new(1, 0)),
        "middle lines are covered whole"
    );
    assert!(reading_order.contains(GridPoint::new(2, 3)));
    assert!(!reading_order.contains(GridPoint::new(2, 4)));

    let block = SelectionRange {
        start: GridPoint::new(2, 4),
        end: GridPoint::new(4, 8),
        block: true,
    };
    assert!(block.contains(GridPoint::new(3, 6)));
    assert!(
        !block.contains(GridPoint::new(3, 2)),
        "a block keeps its columns"
    );
    assert!(!block.contains(GridPoint::new(5, 6)));
}

#[test]
fn search_finds_matches_in_both_directions() {
    let terminal = terminal(20, 3, 0);
    terminal.write(b"foo bar foo");
    terminal.snapshot();

    let mut search = Search::new("foo").expect("pattern compiles");

    let first = terminal
        .find_next(&mut search, GridPoint::new(0, 0), SearchDirection::Forward)
        .expect("first match");
    assert_eq!(first.start, GridPoint::new(0, 0));
    assert_eq!(first.end, GridPoint::new(0, 2));

    let second = terminal
        .find_next(&mut search, GridPoint::new(0, 1), SearchDirection::Forward)
        .expect("second match");
    assert_eq!(second.start, GridPoint::new(0, 8));

    let previous = terminal
        .find_next(&mut search, GridPoint::new(0, 3), SearchDirection::Backward)
        .expect("previous match");
    assert_eq!(previous.start, GridPoint::new(0, 0));
}

#[test]
fn search_finds_text_in_the_scrollback() {
    let terminal = terminal(20, 4, 100);
    terminal.write(b"needle in history\r\nmore output\r\nand more\r\nlast\r\n");
    terminal.snapshot();

    let mut search = Search::new("needle").expect("pattern compiles");
    let found = terminal
        .find_next(&mut search, GridPoint::new(-1, 0), SearchDirection::Forward)
        .expect("match in the scrollback");

    assert!(
        found.start.line < 0,
        "the match is above the screen: {found:?}"
    );
}

#[test]
fn a_search_match_range_contains_the_cells_it_spans() {
    let range = GridRange::new(GridPoint::new(0, 3), GridPoint::new(1, 2));

    assert!(range.contains(GridPoint::new(0, 3)));
    assert!(
        range.contains(GridPoint::new(1, 0)),
        "the wrapped part of the match is covered"
    );
    assert!(!range.contains(GridPoint::new(0, 2)));
    assert!(!range.contains(GridPoint::new(1, 3)));
}

#[test]
fn an_invalid_search_pattern_is_an_error_not_a_panic() {
    let error = match Search::new("[") {
        Ok(_) => panic!("an unterminated character class must not compile"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("invalid search pattern"));
}

// ---------------------------------------------------------------------------
// Threading
// ---------------------------------------------------------------------------

#[test]
fn the_model_can_be_shared_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Terminal>();
    assert_send_sync::<Snapshot>();
}

#[test]
fn writing_and_taking_frames_from_two_threads_do_not_deadlock() {
    let terminal = Arc::new(terminal(20, 6, 100));
    let (done_sender, done_receiver) = mpsc::channel();

    let writer = {
        let terminal = Arc::clone(&terminal);
        std::thread::spawn(move || {
            for index in 0..5_000 {
                terminal.write(format!("line {index}\r\n").as_bytes());
                let _ = terminal.drain_intents();
            }
            done_sender.send(()).expect("signal the writer finished");
        })
    };

    let mut frames = 0;
    for _ in 0..500 {
        let frame = terminal.snapshot();
        assert_eq!(frame.line_count(), 6);
        assert_eq!(frame.cell_count(), 20 * 6);
        frames += 1;
    }

    writer.join().expect("writer thread");
    done_receiver.recv().expect("writer signalled completion");
    assert_eq!(frames, 500);
}

// ---------------------------------------------------------------------------
// Crash tests
// ---------------------------------------------------------------------------

/// A deliberately hostile set of inputs: truncated sequences, overlong
/// parameters, malformed strings and invalid UTF-8.
const BOUNDARY_SEQUENCES: &[&[u8]] = &[
    b"\x1b",
    b"\x1b[",
    b"\x1b[1;",
    b"\x1b[?",
    b"\x1b[?2",
    b"\x1b]0;unterminated title",
    b"\x1b]0;title\x07",
    b"\x1b]52;c;*\x07",
    b"\x1b]52;c;!!!not base64!!!\x07",
    b"\x1b]52;c;\x07",
    b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\",
    b"\x1b]8;;unterminated",
    b"\x1bP+q",
    b"\x1bP+q436f\x1b\\",
    b"\x1bP1$r",
    b"\x1b[999999999999999999999999m",
    b"\x1b[38;5;999m",
    b"\x1b[38;2;300;300;300m",
    b"\x1b[58;5;300m",
    b"\x1b[1000000;1000000H",
    b"\x1b[1000000;1000000r",
    b"\x1b[?2026h",
    b"\x1b[?2026h\x1b[?2026l",
    b"\x1b[?1049h",
    b"\x1b[?1049l",
    b"\x1b[?1000h\x1b[?1006h",
    b"\x1b[?1000l\x1b[?1006l",
    b"\x1b[5 q",
    b"\x1b[0 q",
    b"\x1b[?1;2;3;4;5;6;7;8;9;10c",
    b"\x00\x01\x02\x03\x04\x05",
    b"\x7f\x9b\x9c\x9d",
    b"\xff\xfe\x80",
    b"\xf0\x9f",
    b"\xed\xa0\x80",
    b"\x1b[4h\x1b[4l",
    b"\x1b[?7l\x1b[?7h",
    b"\x1b[2J\x1b[H",
    b"\x1b[3;3H\x1b[1K",
    b"\x1b(B\x1b)0",
    b"\x1b#8",
    b"\x1b[18t",
    b"\x1b[14t",
    b"\x1b]11;?\x07",
    b"\x1b]10;?\x07",
];

#[test]
fn boundary_sequences_are_absorbed() {
    let terminal = terminal(12, 4, 32);

    for sequence in BOUNDARY_SEQUENCES {
        // Once as a whole batch, once byte by byte, because a session reader
        // can split anywhere.
        terminal.write(sequence);
        for byte in *sequence {
            terminal.write(&[*byte]);
        }
        let frame = terminal.snapshot();
        assert_eq!(frame.line_count(), 4);
        assert_eq!(frame.cell_count(), 12 * 4);
        let _ = terminal.drain_intents();
    }
}

#[test]
fn overlong_parameters_and_strings_are_absorbed() {
    let terminal = terminal(12, 4, 32);

    let mut parameters = b"\x1b[".to_vec();
    parameters.extend(std::iter::repeat_n(b'9', 200_000));
    parameters.push(b'm');
    terminal.write(&parameters);

    let mut unterminated_osc = b"\x1b]0;".to_vec();
    unterminated_osc.extend(std::iter::repeat_n(b'x', 200_000));
    terminal.write(&unterminated_osc);

    let mut unterminated_dcs = b"\x1bP+q".to_vec();
    unterminated_dcs.extend(std::iter::repeat_n(b'4', 200_000));
    terminal.write(&unterminated_dcs);

    let frame = terminal.snapshot();
    assert_eq!(frame.line_count(), 4);
    assert_eq!(frame.cell_count(), 12 * 4);
}

/// A deterministic SplitMix64 generator, so any failure can be replayed from
/// the seed in the report. No new dependency is needed for this.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next() % bound as u64) as usize
    }
}

/// Bytes that frequently start or continue an escape sequence, mixed with
/// plain text and bytes that are invalid UTF-8.
const HOSTILE_BYTES: &[u8] = b"\x1b[]PpXx0123456789;:?<>=!\"#$%&'()*+,-./\\ \x07\x08\x09\x0a\x0d\x7f\x9b\x9c\xc3\xa9\xe4\xb8\xad\xf0\x9f\xff\xfe\x80abcXYZ";

fn hostile_bytes(rng: &mut Rng, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| HOSTILE_BYTES[rng.below(HOSTILE_BYTES.len())])
        .collect()
}

fn random_resize(terminal: &Terminal, rng: &mut Rng) -> GridSize {
    let grid = GridSize::new(1 + rng.below(40), 1 + rng.below(12));
    let cell = CellSize::new(1 + rng.below(20) as u16, 1 + rng.below(30) as u16);
    terminal.resize(grid, cell);
    grid
}

fn random_scroll_or_select(terminal: &Terminal, rng: &mut Rng) {
    match rng.below(5) {
        0 => terminal.scroll(Scroll::Delta(-(rng.below(40) as i32))),
        1 => terminal.scroll(Scroll::Delta(rng.below(40) as i32)),
        2 => terminal.scroll(Scroll::Top),
        3 => terminal.scroll(Scroll::Bottom),
        _ => {
            let point = GridPoint::new(rng.below(20) as i32 - 10, rng.below(40));
            terminal.select(SelectionKind::Simple, point, Side::Left);
            terminal.extend_selection(point, Side::Right);
            terminal.clear_selection();
        }
    }
}

fn assert_frame_shape(frame: &Snapshot, grid: GridSize) {
    assert_eq!(frame.line_count(), grid.screen_lines);
    assert_eq!(frame.cell_count(), grid.screen_lines * grid.columns);
    assert!(frame.rows().iter().all(|row| row.len() == grid.columns));
    assert!(frame.cursor().point.column < grid.columns);
}

/// P0-09's crash test: a hundred thousand hostile input groups must not
/// panic, and the model must keep producing well-formed frames throughout.
///
/// The seed and the elapsed time are printed so the report can quote them and
/// a failure can be reproduced. Print them with `-- --nocapture`.
#[test]
fn absorbs_a_hundred_thousand_hostile_inputs_without_panicking() {
    const SEED: u64 = 0x0E09_2026_1105_0001;
    const GROUPS: usize = 100_000;

    let started = Instant::now();
    let mut rng = Rng::new(SEED);
    let mut grid = GridSize::new(20, 8);
    let terminal = terminal(grid.columns, grid.screen_lines, 256);

    for group in 0..GROUPS {
        match rng.below(10) {
            0 => grid = random_resize(&terminal, &mut rng),
            1 => random_scroll_or_select(&terminal, &mut rng),
            2..=4 => terminal.write(BOUNDARY_SEQUENCES[rng.below(BOUNDARY_SEQUENCES.len())]),
            _ => {
                let length = 1 + rng.below(96);
                let bytes = hostile_bytes(&mut rng, length);
                terminal.write(&bytes);
            }
        }
        let _ = terminal.drain_intents();

        if group % 1_024 == 0 {
            let frame = terminal.snapshot();
            assert_frame_shape(&frame, grid);
        }
    }

    let elapsed = started.elapsed();
    eprintln!(
        "hostile input crash test: seed={SEED:#x} groups={GROUPS} elapsed={elapsed:.2?} \
         grid={grid:?}",
    );
}
