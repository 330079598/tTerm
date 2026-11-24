//! Behaviour tests for the retained-history search model.
//!
//! The model tests drive the same public API a worker will use. The pure tests
//! build [`RetainedRows`] by hand to check chunk boundaries, cancellation and
//! generation changes without depending on parser timing.

use super::{
    RetainedRows, SEARCH_CHUNK_ROWS, SearchCancellation, SearchOptions, SearchOutcome,
    SearchPosition, SearchQuery, SearchResult, SearchRow, UnitPosition, scan_chunks,
};
use crate::grid::{CellSize, GridSize};
use crate::terminal::{Config, Terminal};

const CELL: CellSize = CellSize::new(8, 16);

fn model(columns: usize, lines: usize, scrollback: usize) -> Terminal {
    Terminal::new(
        Config {
            scrollback,
            ..Config::default()
        },
        GridSize::new(columns, lines),
        CELL,
    )
}

fn run(model: &Terminal, query: &str, options: SearchOptions) -> SearchResult {
    let query = SearchQuery::new(query, options).expect("the query compiles");
    let cancellation = SearchCancellation::new();
    match model.search_retained(&query, &cancellation) {
        SearchOutcome::Complete(result) => result,
        other => panic!("expected a complete scan, got {other:?}"),
    }
}

fn starts(result: &SearchResult) -> Vec<(i64, usize)> {
    result
        .matches()
        .iter()
        .map(|matched| (matched.start.line, matched.start.column))
        .collect()
}

fn ends(result: &SearchResult) -> Vec<(i64, usize)> {
    result
        .matches()
        .iter()
        .map(|matched| (matched.end.line, matched.end.column))
        .collect()
}

#[test]
fn an_empty_query_is_complete_and_empty() {
    let terminal = model(20, 3, 10);
    terminal.write(b"needle\r\n");

    let result = run(&terminal, "", SearchOptions::default());

    assert_eq!(result.match_count(), 0);
    assert!(result.is_empty());
    assert_eq!(
        result.generation(),
        terminal.snapshot().scrollback_generation()
    );
    assert_eq!(result.content_version(), terminal.content_version());
}

#[test]
fn literal_mode_escapes_regex_syntax_and_regex_mode_does_not() {
    let terminal = model(20, 3, 10);
    terminal.write(b"a.b axb\r\n");

    let literal = run(&terminal, "a.b", SearchOptions::default());
    assert_eq!(starts(&literal), vec![(0, 0)]);
    assert_eq!(ends(&literal), vec![(0, 2)]);

    let regex = run(
        &terminal,
        "a.b",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(starts(&regex), vec![(0, 0), (0, 4)]);
}

#[test]
fn an_invalid_regular_expression_is_an_error_not_literal_text() {
    let error = SearchQuery::new(
        "[",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    )
    .expect_err("an unterminated class must not compile");

    assert!(
        error.to_string().contains("invalid search pattern"),
        "the error must be displayable: {error}"
    );
    assert!(!error.message().is_empty());

    let literal = SearchQuery::new("[", SearchOptions::default()).expect("literal brackets work");
    assert!(!literal.is_empty());
}

#[test]
fn case_sensitivity_is_an_explicit_option() {
    let terminal = model(30, 3, 10);
    terminal.write(b"Needle needle NEEDLE\r\n");

    let folded = run(&terminal, "needle", SearchOptions::default());
    assert_eq!(folded.match_count(), 3);

    let exact = run(
        &terminal,
        "needle",
        SearchOptions {
            case_sensitive: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(starts(&exact), vec![(0, 7)]);
}

#[test]
fn a_query_with_no_matches_is_a_complete_empty_result() {
    let terminal = model(20, 3, 10);
    terminal.write(b"alpha beta\r\n");

    let result = run(&terminal, "gamma", SearchOptions::default());

    assert!(result.is_empty());
    assert_eq!(result.match_count(), 0);
    assert!(result.total_lines() > 0);
}

#[test]
fn unicode_case_folding_follows_the_i_flag() {
    let terminal = model(20, 3, 10);
    terminal.write("ÄÖÜ äöü\r\n".as_bytes());

    let folded = run(&terminal, "äöü", SearchOptions::default());
    assert_eq!(starts(&folded), vec![(0, 0), (0, 4)]);

    let exact = run(
        &terminal,
        "äöü",
        SearchOptions {
            case_sensitive: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(starts(&exact), vec![(0, 4)]);
}

#[test]
fn regular_expressions_keep_ecmascript_lookbehind() {
    let terminal = model(20, 3, 10);
    terminal.write(b"price $42 and 43\r\n");

    let result = run(
        &terminal,
        r"(?<=\$)\d+",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    );

    assert_eq!(starts(&result), vec![(0, 7)]);
    assert_eq!(ends(&result), vec![(0, 8)]);
}

#[test]
fn whole_word_uses_the_frontends_non_word_characters() {
    let terminal = model(40, 3, 10);
    terminal.write(b"needle needles _needle needle- needle.\r\n");

    let result = run(
        &terminal,
        "needle",
        SearchOptions {
            whole_word: true,
            ..SearchOptions::default()
        },
    );

    assert_eq!(starts(&result), vec![(0, 0), (0, 23), (0, 31)]);
}

#[test]
fn whole_word_keeps_the_frontends_non_ascii_behavior() {
    let terminal = model(30, 3, 10);
    terminal.write("中文 中 中文\r\n".as_bytes());

    let whole = run(
        &terminal,
        "中",
        SearchOptions {
            whole_word: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(
        starts(&whole),
        vec![(0, 5)],
        "a CJK letter is not one of the frontend's non-word characters"
    );

    let plain = run(&terminal, "中", SearchOptions::default());
    assert_eq!(plain.match_count(), 3);
}

#[test]
fn unicode_and_wide_characters_map_to_their_cells() {
    let terminal = model(30, 3, 10);
    terminal.write("中文 e\u{301}\r\n".as_bytes());

    let wide = run(&terminal, "文", SearchOptions::default());
    assert_eq!(starts(&wide), vec![(0, 2)]);
    assert_eq!(
        ends(&wide),
        vec![(0, 3)],
        "a wide glyph covers both of its columns"
    );

    let combining = run(&terminal, "e\u{301}", SearchOptions::default());
    assert_eq!(starts(&combining), vec![(0, 5)]);
    assert_eq!(ends(&combining), vec![(0, 5)]);
}

#[test]
fn a_match_can_span_wrapped_rows() {
    let terminal = model(5, 3, 10);
    terminal.write(b"abcdefghij");

    let crossing = run(&terminal, "defg", SearchOptions::default());
    assert_eq!(starts(&crossing), vec![(0, 3)]);
    assert_eq!(ends(&crossing), vec![(1, 1)]);

    let whole = run(&terminal, "abcdefghij", SearchOptions::default());
    assert_eq!(starts(&whole), vec![(0, 0)]);
    assert_eq!(ends(&whole), vec![(1, 4)]);
}

#[test]
fn a_wide_character_that_wraps_keeps_its_place() {
    let terminal = model(5, 3, 10);
    terminal.write("abcd中ef".as_bytes());

    let crossing = run(&terminal, "d中", SearchOptions::default());
    assert_eq!(starts(&crossing), vec![(0, 3)]);
    assert_eq!(
        ends(&crossing),
        vec![(1, 1)],
        "the wrapped wide glyph covers columns 0 and 1"
    );

    let wide = run(&terminal, "中e", SearchOptions::default());
    assert_eq!(starts(&wide), vec![(1, 0)]);
    assert_eq!(
        ends(&wide),
        vec![(1, 2)],
        "the wide glyph covers columns 0 and 1"
    );
}

#[test]
fn adjacent_hard_lines_do_not_form_a_wrapped_line() {
    let terminal = model(10, 3, 10);
    terminal.write(b"abcde\r\nfghij\r\n");

    let crossing = run(&terminal, "defg", SearchOptions::default());
    assert!(crossing.is_empty(), "a hard newline must break the text");

    let second = run(&terminal, "fghij", SearchOptions::default());
    assert_eq!(starts(&second), vec![(1, 0)]);
}

#[test]
fn results_are_ordered_and_index_the_next_and_previous_match() {
    let terminal = model(20, 3, 10);
    terminal.write(b"foo foo\r\nfoo\r\n");

    let result = run(&terminal, "foo", SearchOptions::default());
    assert_eq!(starts(&result), vec![(0, 0), (0, 4), (1, 0)]);

    let at = |line, column| SearchPosition { line, column };
    assert_eq!(
        result.index_from(at(0, 0), super::SearchDirection::Forward),
        Some(0)
    );
    assert_eq!(
        result.index_from(at(0, 1), super::SearchDirection::Forward),
        Some(1)
    );
    assert_eq!(
        result.index_from(at(1, 0), super::SearchDirection::Forward),
        Some(2)
    );
    assert_eq!(
        result.index_from(at(1, 1), super::SearchDirection::Forward),
        None
    );
    assert_eq!(
        result.index_from(at(1, 1), super::SearchDirection::Backward),
        Some(2)
    );
    assert_eq!(
        result.index_from(at(0, 4), super::SearchDirection::Backward),
        Some(1)
    );
    assert_eq!(
        result.index_from(at(0, 0), super::SearchDirection::Backward),
        Some(0)
    );
    assert_eq!(
        result.index_from(at(-1, 0), super::SearchDirection::Backward),
        None
    );
}

#[test]
fn a_regular_expression_that_matches_nothing_adds_no_empty_result() {
    let terminal = model(20, 3, 10);
    terminal.write(b"abc\r\n");

    let anchored = run(
        &terminal,
        "^",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    );
    assert!(anchored.is_empty());

    let star = run(
        &terminal,
        "a*",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(
        starts(&star),
        vec![(0, 0)],
        "the non-empty part matches once and empty matches are skipped"
    );
}

#[test]
fn rows_copied_for_a_worker_are_bounded() {
    let terminal = model(20, 3, 2_000);
    let mut bytes = Vec::new();
    for index in 0..2_000 {
        bytes.extend_from_slice(format!("line-{index:04}\r\n").as_bytes());
    }
    terminal.write(&bytes);

    let rows = terminal.retained_rows(0, usize::MAX);
    assert_eq!(rows.rows().len(), SEARCH_CHUNK_ROWS);
    assert_eq!(rows.first_line(), 0);
    assert_eq!(rows.content_version(), terminal.content_version());
    assert!(rows.total_lines() > SEARCH_CHUNK_ROWS);
}

#[test]
fn a_long_buffer_is_scanned_in_chunks_without_missing_matches() {
    let terminal = model(20, 3, 5_000);
    let mut bytes = Vec::new();
    for index in 0..5_000 {
        bytes.extend_from_slice(format!("needle {index:04}\r\n").as_bytes());
    }
    terminal.write(&bytes);

    let result = run(&terminal, "needle", SearchOptions::default());
    assert_eq!(result.match_count(), 5_000);
    assert_eq!(result.matches()[0].start.line, 0);
    let last = run(&terminal, "needle 4999", SearchOptions::default());
    assert_eq!(last.match_count(), 1);
    assert_eq!(
        result.matches().last().expect("a last match").start.line,
        last.matches()[0].start.line
    );
}

#[test]
fn a_generation_change_discards_the_partial_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        let generation = if start == 0 { 7 } else { 8 };
        RetainedRows::new(
            generation,
            1,
            2,
            start,
            vec![row(start as i64, "needle", false)],
        )
    });

    assert_eq!(outcome, SearchOutcome::Stale);
}

#[test]
fn a_write_between_chunks_discards_the_partial_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let mut fetches = 0;
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        fetches += 1;
        let content_version = if start == 0 { 3 } else { 4 };
        RetainedRows::new(
            9,
            content_version,
            2,
            start,
            vec![row(start as i64, "needle", false)],
        )
    });

    assert_eq!(outcome, SearchOutcome::Stale);
    assert_eq!(
        fetches, 2,
        "the scan stops at the chunk from another version"
    );
}

#[test]
fn an_in_place_overwrite_between_chunks_discards_the_partial_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        // The row count and scrollback generation stay put; only the cells
        // on the second row are rewritten.
        let (content_version, text) = if start == 0 {
            (5, "needle")
        } else {
            (6, "XXXXXX")
        };
        RetainedRows::new(
            1,
            content_version,
            2,
            start,
            vec![row(start as i64, text, false)],
        )
    });

    assert_eq!(outcome, SearchOutcome::Stale);
}

#[test]
fn an_untrimmed_append_seen_only_by_the_final_check_discards_the_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        if start == 0 {
            RetainedRows::new(1, 1, 1, 0, vec![row(0, "needle", false)])
        } else {
            // The append does not evict, so the generation and total do not
            // move; only the content version notices it.
            RetainedRows::new(1, 2, 2, start, Vec::new())
        }
    });

    assert_eq!(outcome, SearchOutcome::Stale);
}

#[test]
fn an_untrimmed_append_between_chunks_discards_the_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        let total_lines = if start == 0 { 2 } else { 3 };
        RetainedRows::new(
            1,
            1,
            total_lines,
            start,
            vec![row(start as i64, "needle", false)],
        )
    });

    assert_eq!(outcome, SearchOutcome::Stale);
}

#[test]
fn a_chunk_that_runs_past_the_sampled_total_discards_the_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        let rows = if start == 0 {
            vec![row(0, "needle", false)]
        } else {
            vec![row(1, "needle", false), row(2, "needle", false)]
        };
        RetainedRows::new(1, 1, 2, start, rows)
    });

    assert_eq!(
        outcome,
        SearchOutcome::Stale,
        "a result must never report fewer total lines than it scanned"
    );
}

#[test]
fn a_write_during_the_final_check_discards_the_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        if start == 0 {
            RetainedRows::new(1, 1, 1, 0, vec![row(0, "needle", false)])
        } else {
            RetainedRows::new(1, 2, 1, start, Vec::new())
        }
    });

    assert_eq!(outcome, SearchOutcome::Stale);
}

#[test]
fn the_final_cancellation_check_wins_over_a_newer_content_version() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let shared = cancellation.clone();
    let outcome = scan_chunks(&query, &cancellation, move |start, _| {
        if start == 0 {
            RetainedRows::new(1, 1, 1, 0, vec![row(0, "needle", false)])
        } else {
            shared.cancel();
            RetainedRows::new(1, 2, 1, start, Vec::new())
        }
    });

    assert_eq!(
        outcome,
        SearchOutcome::Cancelled,
        "a cancelled scan must not report a result, even when the content also changed"
    );
}

#[test]
fn a_pre_cancelled_scan_does_not_fetch_rows() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    cancellation.cancel();
    let mut fetched = false;
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        fetched = true;
        RetainedRows::new(1, 1, 0, start, Vec::new())
    });

    assert_eq!(outcome, SearchOutcome::Cancelled);
    assert!(
        !fetched,
        "cancellation must be observed before the model lock"
    );
}

#[test]
fn cancellation_stops_the_scan() {
    let query = SearchQuery::new("needle", SearchOptions::default()).expect("query compiles");
    let cancellation = SearchCancellation::new();
    let shared = cancellation.clone();
    let outcome = scan_chunks(&query, &cancellation, |start, _| {
        shared.cancel();
        RetainedRows::new(1, 1, 2, start, vec![row(start as i64, "needle", false)])
    });

    assert_eq!(outcome, SearchOutcome::Cancelled);
}

#[test]
fn an_empty_query_does_not_copy_the_buffer() {
    let terminal = model(20, 3, 5_000);
    let mut bytes = Vec::new();
    for index in 0..5_000 {
        bytes.extend_from_slice(format!("line-{index:04}\r\n").as_bytes());
    }
    terminal.write(&bytes);

    let rows = terminal.retained_rows(0, 0);
    assert!(rows.rows().is_empty());
    assert!(rows.total_lines() > 0);
}

#[test]
fn the_search_types_can_move_to_a_worker() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SearchQuery>();
    assert_send_sync::<SearchCancellation>();
    assert_send_sync::<SearchResult>();
    assert_send_sync::<RetainedRows>();
    assert_send_sync::<Terminal>();
}

fn row(line: i64, text: &str, wrapped: bool) -> SearchRow {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut positions = Vec::with_capacity(units.len());
    for (column, character) in text.chars().enumerate() {
        let position = UnitPosition::new(column, 1).expect("the test column fits");
        for _ in 0..character.len_utf16() {
            positions.push(position);
        }
    }
    SearchRow::new(line, units, positions, wrapped)
}
