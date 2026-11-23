//! Clear-history behaviour through the public model API.
//!
//! PTY reads are not semantic boundaries: the same bytes must produce the same
//! grid whether they arrive in one write or in single-byte writes. These tests
//! feed both shapes and compare the visible frame, history length, display
//! offset, mode bits and history generation where the generation is itself a
//! clear result rather than a write-boundary side effect.

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::{Config, Terminal};

const COLUMNS: usize = 20;
const LINES: usize = 4;
const CAPACITY: usize = 100;

fn model() -> Terminal {
    Terminal::new(
        Config {
            scrollback: CAPACITY,
            ..Config::default()
        },
        GridSize::new(COLUMNS, LINES),
        CellSize::new(8, 16),
    )
}

fn short_model() -> Terminal {
    Terminal::new(
        Config {
            scrollback: CAPACITY,
            ..Config::default()
        },
        GridSize::new(COLUMNS, 2),
        CellSize::new(8, 16),
    )
}

fn numbered(prefix: &str, count: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for index in 0..count {
        bytes.extend_from_slice(format!("{prefix}{index:02}\r\n").as_bytes());
    }
    bytes
}

#[derive(Debug, PartialEq, Eq)]
struct Frame {
    rows: Vec<String>,
    history: usize,
    offset: usize,
    generation: u64,
    alternate: bool,
}

fn frame(model: &Terminal) -> Frame {
    let snapshot = model.snapshot();
    Frame {
        rows: snapshot
            .rows()
            .iter()
            .map(|row| row.cells().iter().map(|cell| cell.ch).collect())
            .collect(),
        history: snapshot.history_size(),
        offset: snapshot.display_offset(),
        generation: snapshot.scrollback_generation(),
        alternate: snapshot.modes().contains(Modes::ALT_SCREEN),
    }
}

fn write_in_chunks(model: &Terminal, bytes: &[u8], chunk: Option<usize>) {
    match chunk {
        Some(size) => {
            for part in bytes.chunks(size) {
                model.write(part);
            }
        }
        None => model.write(bytes),
    }
}

fn feed(bytes: &[u8], chunk: Option<usize>) -> Frame {
    let model = model();
    write_in_chunks(&model, bytes, chunk);
    frame(&model)
}

fn feed_short(bytes: &[u8], chunk: Option<usize>) -> Frame {
    let model = short_model();
    write_in_chunks(&model, bytes, chunk);
    frame(&model)
}

#[test]
fn a_standalone_saved_line_clear_keeps_the_visible_rows() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);
    assert!(before.history > 0);

    model.write(b"\x1b[3J");
    let after = frame(&model);

    assert_eq!(after.rows, before.rows, "ED 3 clears saved lines only");
    assert_eq!(after.history, 0);
    assert_eq!(after.offset, 0);
    assert!(
        after.generation > before.generation,
        "the clear must invalidate absolute-line caches"
    );
}

#[test]
fn clear_and_fresh_output_match_whether_the_stream_is_one_write_or_split() {
    let mut stream = numbered("old-", 10);
    stream.extend_from_slice(b"\x1b[H\x1b[2J\x1b[3J");
    stream.extend_from_slice(&numbered("fresh-", 12));

    let once = feed(&stream, None);
    let split = feed(&stream, Some(1));

    assert_eq!(once, split);
    assert!(once.history > 0, "fresh output refills the history");
    assert!(
        once.rows.iter().any(|line| line.contains("fresh-11")),
        "the newest output is visible"
    );
    assert!(
        once.rows.iter().all(|line| !line.contains("old-")),
        "no old line survives the clear"
    );
}

#[test]
fn a_clear_split_inside_its_escape_sequence_runs_when_completed() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b[");
    assert_eq!(frame(&model).history, before.history);
    model.write(b"3");
    assert_eq!(frame(&model).history, before.history);
    model.write(b"J");
    let after = frame(&model);

    assert_eq!(after.history, 0);
    assert!(after.generation > before.generation);
    assert_eq!(after.rows, before.rows);

    let whole = feed(b"\x1b[3J", None);
    let split = feed(b"\x1b[3J", Some(1));
    assert_eq!(whole, split);
}

#[test]
fn a_clear_before_the_alternate_screen_switch_is_kept_on_the_main_screen() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b[3J\x1b[?1049h");
    let alternate = frame(&model);
    assert!(alternate.alternate);
    assert_eq!(alternate.history, 0);

    model.write(b"\x1b[?1049l");
    let back = frame(&model);

    assert!(!back.alternate);
    assert_eq!(back.rows, before.rows, "the main screen keeps its rows");
    assert_eq!(back.history, 0, "the clear happened before the switch");
    assert!(back.generation > before.generation);

    let mut stream = numbered("old-", 10);
    stream.extend_from_slice(b"\x1b[3J\x1b[?1049h");
    stream.extend_from_slice(b"\x1b[?1049l");
    let once = feed(&stream, None);
    let split = feed(&stream, Some(1));
    assert_eq!(once.rows, split.rows);
    assert_eq!(once.history, split.history);
    assert_eq!(once.offset, split.offset);
    assert_eq!(once.alternate, split.alternate);
}

#[test]
fn a_clear_on_the_alternate_screen_does_not_touch_the_main_history() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b[?1049h");
    model.write(b"\x1b[H\x1b[2J\x1b[3J");
    let alternate = frame(&model);
    assert!(alternate.alternate);
    assert_eq!(alternate.history, 0);

    model.write(b"\x1b[?1049l");
    let back = frame(&model);

    assert_eq!(back.rows, before.rows);
    assert_eq!(
        back.history, before.history,
        "an alternate-screen clear must not clear the hidden main history"
    );
}

#[test]
fn text_that_looks_like_a_clear_inside_control_strings_is_ignored() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b]0;a title with 3J inside\x07");
    model.write(b"\x1bP1;2|a DCS with 3J inside\x1b\\");
    model.write(b"\x1b]8;;https://example.com/3J\x07\x1b]8;;\x07");
    model.write(b"\x1b[?3J");

    let after = frame(&model);
    assert_eq!(after.history, before.history);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.rows, before.rows);
}

#[test]
fn a_synchronized_saved_line_clear_runs_at_the_end_of_the_update() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b[?2026h\x1b[3J");
    let buffered = frame(&model);
    assert_eq!(buffered.history, before.history);
    assert_eq!(buffered.generation, before.generation);
    assert_eq!(buffered.rows, before.rows);

    model.write(b"\x1b[?2026l");
    let after = frame(&model);
    assert_eq!(after.history, 0);
    assert!(after.generation > before.generation);
    assert_eq!(after.rows, before.rows);
}

#[test]
fn a_synchronized_reset_runs_at_the_end_of_the_update() {
    let model = model();
    model.write(&numbered("old-", 10));
    let before = frame(&model);

    model.write(b"\x1b[?2026h\x1bc");
    let buffered = frame(&model);
    assert_eq!(buffered.history, before.history);
    assert_eq!(buffered.generation, before.generation);

    model.write(b"\x1b[?2026l");
    let after = frame(&model);
    assert_eq!(after.history, 0);
    assert!(after.generation > before.generation);
    assert!(
        after
            .rows
            .iter()
            .all(|line| line.chars().all(|ch| ch == ' ')),
        "the reset clears the visible screen too"
    );
}

#[test]
fn a_reset_that_refills_the_same_history_still_advances_the_generation() {
    // Five lines on a two-line screen leave four history lines. The reset and
    // another five lines leave the same length, so a history comparison alone
    // would miss the clear.
    let mut stream = numbered("old-", 5);
    stream.push(0x1b);
    stream.push(b'c');
    stream.extend_from_slice(&numbered("new-", 5));

    let once = feed_short(&stream, None);
    let split = feed_short(&stream, Some(1));

    assert_eq!(once, split);
    assert_eq!(once.history, 4);
    assert!(
        once.generation > 0,
        "the reset must advance the generation even when output refills it"
    );
    assert!(
        once.rows.iter().any(|line| line.contains("new-04")),
        "frames: once={once:?} split={split:?}"
    );
    assert!(
        once.rows.iter().all(|line| !line.contains("old-")),
        "the reset drops the old screen content"
    );
}
