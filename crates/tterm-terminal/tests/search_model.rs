//! Lifecycle tests for the retained-history search model.
//!
//! The scenarios use the production model and synthetic output. They assert
//! the coordinate P2-06's overview marks use, and that eviction, a clear, the
//! alternate screen and resize invalidate a result instead of leaving it
//! pointing at another row.

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::search::{
    SearchCancellation, SearchOptions, SearchOutcome, SearchQuery, SearchResult,
};
use tterm_terminal::terminal::{Config, Scroll, Terminal};

const COLUMNS: usize = 20;

fn model(lines: usize, capacity: usize) -> Terminal {
    Terminal::new(
        Config {
            scrollback: capacity,
            ..Config::default()
        },
        GridSize::new(COLUMNS, lines),
        CellSize::new(8, 16),
    )
}

fn command(model: &Terminal, index: usize, status: i64) {
    model.write(format!("\x1b]133;A\x07$ cmd{index}\r\n").as_bytes());
    model.write(b"\x1b]133;C\x07");
    model.write(format!("out{index}\r\n").as_bytes());
    model.write(format!("\x1b]133;D;{status}\x07").as_bytes());
}

fn run(model: &Terminal, query: &str) -> SearchResult {
    let query = SearchQuery::new(query, SearchOptions::default()).expect("the query compiles");
    let cancellation = SearchCancellation::new();
    match model.search_retained(&query, &cancellation) {
        SearchOutcome::Complete(result) => result,
        other => panic!("expected a complete scan, got {other:?}"),
    }
}

fn screen_row(model: &Terminal, needle: &str) -> usize {
    let snapshot = model.snapshot();
    snapshot
        .rows()
        .iter()
        .position(|row| {
            row.cells()
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
                .contains(needle)
        })
        .expect("the text is visible")
}

#[test]
fn search_lines_use_the_same_coordinate_as_overview_command_marks() {
    let model = model(5, 50);
    for index in 0..3 {
        command(&model, index, 0);
    }
    let marks = model.snapshot().command_marks().to_vec();
    assert_eq!(marks.len(), 3);

    let prompts = run(&model, "cmd");
    assert_eq!(prompts.match_count(), 3);
    for (matched, mark) in prompts.matches().iter().zip(&marks) {
        assert_eq!(
            matched.start.line, mark.prompt_line,
            "a prompt match and its overview mark must name the same row"
        );
    }

    let output = run(&model, "out");
    assert_eq!(output.match_count(), 3);
    for (matched, mark) in output.matches().iter().zip(&marks) {
        assert_eq!(
            matched.start.line,
            mark.output_line.expect("the shell marked output"),
            "an output match and its overview mark must name the same row"
        );
    }

    let snapshot = model.snapshot();
    let last = run(&model, "out2");
    let row = screen_row(&model, "out2");
    assert_eq!(
        last.matches()[0].start.line,
        snapshot.history_size() as i64 + row as i64,
        "the last visible row is history_size + screen_row"
    );
}

#[test]
fn output_growth_keeps_existing_lines_and_adds_new_ones() {
    let model = model(5, 50);
    for index in 0..5 {
        model.write(format!("row-{index}\r\n").as_bytes());
    }
    let before = run(&model, "row-0");
    assert_eq!(before.match_count(), 1);
    let line = before.matches()[0].start.line;
    let generation = before.generation();

    for index in 5..15 {
        model.write(format!("row-{index}\r\n").as_bytes());
    }
    let after = run(&model, "row-0");
    assert_eq!(
        after.generation(),
        generation,
        "plain output growth does not evict a row"
    );
    assert_eq!(
        after.matches()[0].start.line,
        line,
        "the retained coordinate of an existing line must not move"
    );

    let newest = run(&model, "row-14");
    assert_eq!(newest.match_count(), 1);
    assert!(
        newest.matches()[0].start.line > line,
        "new output appends after the rows already scanned"
    );
}

#[test]
fn eviction_advances_the_generation_and_drops_old_matches() {
    let model = model(3, 20);
    let mut bytes = Vec::new();
    for index in 0..10 {
        bytes.extend_from_slice(format!("evict-{index}\r\n").as_bytes());
    }
    model.write(&bytes);
    let before = run(&model, "evict-0");
    assert_eq!(before.match_count(), 1);

    let mut more = Vec::new();
    for index in 10..40 {
        more.extend_from_slice(format!("evict-{index}\r\n").as_bytes());
    }
    model.write(&more);

    let after = run(&model, "evict-0");
    assert!(
        after.generation() > before.generation(),
        "evicting the oldest rows must advance the generation"
    );
    assert!(
        after.is_empty(),
        "the match whose row was evicted cannot survive the rescan"
    );
}

#[test]
fn clearing_the_history_advances_the_generation() {
    let model = model(3, 20);
    model.write(b"needle\r\n");
    let before = run(&model, "needle");
    assert_eq!(before.match_count(), 1);

    model.write(b"\x1b[H\x1b[2J\x1b[3J");
    let after = run(&model, "needle");

    assert!(
        after.generation() > before.generation(),
        "a clear must invalidate cached rows"
    );
    assert!(after.is_empty(), "the cleared text is gone");
}

#[test]
fn alternate_screen_switches_invalidate_and_rescan_the_active_grid() {
    let model = model(10, 20);
    model.write(b"needle main\r\n");
    let main = run(&model, "needle");
    assert_eq!(main.match_count(), 1);

    model.write(b"\x1b[?1049h");
    model.write(b"\x1b[H");
    model.write(b"needle alternate\r\n");
    let alternate = run(&model, "needle");
    assert!(
        alternate.generation() > main.generation(),
        "entering the alternate screen swaps the visible grid"
    );
    assert_eq!(alternate.match_count(), 1);
    assert_eq!(
        alternate.matches()[0].start.line,
        0,
        "the alternate grid has its own retained coordinate"
    );

    model.write(b"\x1b[?1049l");
    let back = run(&model, "needle main");
    assert!(
        back.generation() > alternate.generation(),
        "leaving the alternate screen restores another grid"
    );
    assert_eq!(back.match_count(), 1);
    assert_eq!(back.matches()[0].start.line, main.matches()[0].start.line);
}

#[test]
fn resize_invalidates_and_rescans_reflowed_rows() {
    let model = model(20, 6);
    model.write(b"alpha\r\nabcdefghijklmnopqrst\r\nomega-needle\r\n");
    let before = run(&model, "omega-needle");
    assert_eq!(before.match_count(), 1);
    let line = before.matches()[0].start.line;

    model.resize(GridSize::new(10, 6), CellSize::new(8, 16));
    let after = run(&model, "omega-needle");

    assert!(
        after.generation() > before.generation(),
        "reflow moves rows, so cached retained lines must expire"
    );
    assert_eq!(after.match_count(), 1);
    assert!(
        after.matches()[0].start.line > line,
        "the long line above the marker now occupies more rows"
    );
}

#[test]
fn scrolling_does_not_move_a_match() {
    let model = model(3, 20);
    let mut bytes = Vec::new();
    for index in 0..10 {
        bytes.extend_from_slice(format!("scroll-{index}\r\n").as_bytes());
    }
    model.write(&bytes);
    let before = run(&model, "scroll-0");

    model.scroll(Scroll::Top);
    let top = run(&model, "scroll-0");
    assert_eq!(top.matches(), before.matches());

    model.scroll(Scroll::Bottom);
    let bottom = run(&model, "scroll-0");
    assert_eq!(bottom.matches(), before.matches());
}

#[test]
fn a_cancelled_search_returns_cancelled_without_a_result() {
    let model = model(3, 20);
    model.write(b"needle\r\n");
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("the query compiles");
    let cancellation = SearchCancellation::new();
    cancellation.cancel();

    assert_eq!(
        model.search_retained(&query, &cancellation),
        SearchOutcome::Cancelled
    );
}
