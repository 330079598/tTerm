//! Search overlays through the production view, model and frame pipeline.
//!
//! The output is synthetic and the scans are the real retained-history API.
//! Assertions read the frame the view prepared and the counters it publishes,
//! not a separately built snapshot or a painted image.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use super::input_tests::Harness;
use super::search::SearchOverlay;
use gpui_kit::TestAppContext;
use tterm_terminal::grid::GridPoint;
use tterm_terminal::search::{
    SearchCancellation, SearchOptions, SearchOutcome, SearchQuery, SearchResult,
};
use tterm_terminal::selection::{SelectionKind, Side};

fn fill(h: &Harness, cx: &mut TestAppContext, lines: usize) {
    let mut bytes = Vec::new();
    for index in 0..lines {
        bytes.extend_from_slice(format!("needle line-{index:03}\r\n").as_bytes());
    }
    h.output(cx, &bytes);
}

fn scan(h: &Harness, query: &str) -> Arc<SearchResult> {
    let query = SearchQuery::new(query, SearchOptions::default()).expect("the query compiles");
    let cancellation = SearchCancellation::new();
    match h.terminal.search_retained(&query, &cancellation) {
        SearchOutcome::Complete(result) => Arc::new(result),
        other => panic!("expected a complete scan, got {other:?}"),
    }
}

fn install(h: &Harness, cx: &mut TestAppContext, result: Arc<SearchResult>, active: Option<usize>) {
    h.update(cx, |view, _, cx| {
        view.set_search_overlay(SearchOverlay::new(result, active), cx);
    });
    h.draw(cx);
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

fn stats(h: &Harness, cx: &mut TestAppContext) -> super::ViewStats {
    h.view.read_with(cx, |view, _| view.stats())
}

#[gpui_kit::test]
fn visible_matches_paint_normal_and_active_spans_and_feed_the_ruler(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    fill(&h, cx, 120);
    let result = scan(&h, "needle");
    install(&h, cx, Arc::clone(&result), Some(0));
    let first_line = result.matches()[0].start.line;
    h.update(cx, |view, _, cx| view.reveal_search_line(first_line, cx));
    h.draw(cx);

    let painted = stats(&h, cx);
    assert!(painted.search_spans > 0, "a visible match is painted");
    assert!(
        painted.search_active_spans > 0,
        "the active match is painted"
    );
    assert!(painted.overview_marks > 0, "the ruler paints the matches");
    assert_eq!(
        h.view.read_with(cx, |view, _| view.search_marks().len()),
        result.match_count()
    );
    assert!(h.view.read_with(cx, |view, _| {
        view.search_marks().iter().any(|mark| mark.active)
    }));

    install(&h, cx, Arc::clone(&result), Some(result.match_count() - 1));
    let last_line = result.matches()[result.match_count() - 1].start.line;
    h.update(cx, |view, _, cx| view.reveal_search_line(last_line, cx));
    h.draw(cx);
    assert!(stats(&h, cx).search_active_spans > 0);
    assert!(
        h.view.read_with(cx, |view, _| view
            .search_marks()
            .iter()
            .filter(|mark| mark.active)
            .count())
            == 1
    );
}

#[gpui_kit::test]
fn output_drops_the_overlay_before_the_next_frame_and_reports_it(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    fill(&h, cx, 20);
    let fired = Rc::new(Cell::new(false));
    let listener = Rc::clone(&fired);
    h.update(cx, |view, _, cx| {
        view.set_search_stale_listener(Some(Rc::new(move |_| listener.set(true))), cx);
    });
    install(&h, cx, scan(&h, "needle"), Some(0));
    assert!(stats(&h, cx).search_spans > 0);
    assert!(!fired.get());

    h.output(cx, b"more output\r\n");
    let painted = stats(&h, cx);
    assert_eq!(painted.search_spans, 0);
    assert_eq!(painted.search_stale_clears, 1);
    assert!(fired.get(), "the owner is told to rescan");
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_overlay().is_empty())
    );
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_marks().is_empty())
    );
}

#[gpui_kit::test]
fn a_large_result_is_clipped_to_the_viewport_and_selection_stays_independent(
    cx: &mut TestAppContext,
) {
    let h = Harness::with_scrollback(cx, 6_000);
    fill(&h, cx, 3_000);
    h.terminal
        .select(SelectionKind::Simple, GridPoint::new(0, 0), Side::Left);
    h.terminal
        .extend_selection(GridPoint::new(0, 8), Side::Right);
    let result = scan(&h, "needle");
    assert!(result.match_count() > 1_000);
    install(&h, cx, Arc::clone(&result), Some(result.match_count() - 1));

    let painted = stats(&h, cx);
    let lines = snapshot(&h, cx).grid().screen_lines;
    assert!(
        painted.search_matches_visited <= lines as u64,
        "the frame visits only matches in the viewport"
    );
    assert!(painted.search_spans <= lines as u64);
    let frame = h.view.read_with(cx, |view, _| {
        view.last_frame.as_ref().expect("a frame").snapshot.clone()
    });
    assert!(frame.selection().is_some(), "a selection is independent");

    h.terminal.clear_selection();
    h.draw(cx);
    assert!(
        stats(&h, cx).search_spans > 0,
        "search survives clearing the selection"
    );
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_overlay().active_index())
            == Some(result.match_count() - 1)
    );
}

#[gpui_kit::test]
fn revealing_an_off_screen_match_scrolls_it_into_the_viewport(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    fill(&h, cx, 120);
    let result = scan(&h, "needle");
    let target = result.matches()[3];
    install(&h, cx, Arc::clone(&result), Some(3));
    let before = snapshot(&h, cx);
    assert!(target.start.line < before.history_size() as i64);

    let moved = h.update(cx, |view, _, cx| {
        view.reveal_search_line(target.start.line, cx)
    });
    h.draw(cx);
    assert!(moved);
    let frame = snapshot(&h, cx);
    let top = frame.history_size() as i64 - frame.display_offset() as i64;
    assert!(target.start.line >= top);
    assert!(target.start.line < top + frame.grid().screen_lines as i64);
    assert!(stats(&h, cx).search_spans > 0);
}

#[gpui_kit::test]
fn a_hidden_view_keeps_no_search_overlay(cx: &mut TestAppContext) {
    let h = Harness::with_scrollback(cx, 500);
    fill(&h, cx, 20);
    install(&h, cx, scan(&h, "needle"), Some(0));
    h.update(cx, |view, _, cx| view.set_visible(false, cx));
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_overlay().is_empty())
    );
    assert!(
        h.view
            .read_with(cx, |view, _| view.search_marks().is_empty())
    );
}
