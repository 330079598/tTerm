//! Search matches accepted for the view: visible spans, ruler marks and reveal.
//!
//! A [`SearchOverlay`] carries one complete scan result and the index of the
//! active match. The result names the content version and scrollback
//! generation it scanned; the view drops the whole overlay when the current
//! snapshot no longer matches, so matches can never point at another row.
//!
//! Visible spans are clipped to the frame with two binary searches, so a
//! frame never walks every match in a large scrollback. The ruler keeps the
//! newest [`MAX_RULER_MARKS`] matches, the same bound P2-06 uses for command
//! marks.

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::App;
use tterm_terminal::palette::Rgba;
use tterm_terminal::search::{SearchMatch, SearchPosition, SearchResult};
use tterm_terminal::snapshot::Snapshot;

use super::overview::OverviewSearchMark;

/// The fixed match colors from the frontend's `searchTypes.ts`.
pub const MATCH_BACKGROUND: Rgba = Rgba::rgb(0xfa, 0xcc, 0x15);
/// The border of a normal match.
pub const MATCH_BORDER: Rgba = Rgba::rgb(0xfd, 0xe0, 0x47);
/// The background of the active match.
pub const ACTIVE_MATCH_BACKGROUND: Rgba = Rgba::rgb(0xfb, 0x92, 0x3c);
/// The border of the active match.
pub const ACTIVE_MATCH_BORDER: Rgba = Rgba::rgb(0xfd, 0xba, 0x74);
/// The background alpha keeps the glyphs readable under the highlight.
pub const MATCH_BACKGROUND_ALPHA: u8 = 90;
/// The active match is a little stronger than the other matches.
pub const ACTIVE_MATCH_BACKGROUND_ALPHA: u8 = 128;

/// The most matches the overview ruler receives from one result.
///
/// P2-06 caps the source list at this number; keeping the same bound here
/// means a huge result allocates the same ruler input it always did.
pub const MAX_RULER_MARKS: usize = 4096;

/// A called when the view drops an overlay because its content changed.
///
/// The search bar uses it to clear its count and schedule a rescan without the
/// view depending on the bar's entity type.
pub type SearchStaleListener = Rc<dyn Fn(&mut App)>;

/// One highlighted run of cells in a visible row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleSearchSpan {
    /// The first column.
    pub start: usize,
    /// The last column, inclusive.
    pub end: usize,
    /// Whether this span belongs to the active match.
    pub active: bool,
}

/// The clipped matches of one frame, one list per visible row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VisibleSearch {
    rows: Vec<Vec<VisibleSearchSpan>>,
    visited: usize,
}

impl VisibleSearch {
    /// The spans of one visible row.
    pub fn row(&self, row: usize) -> &[VisibleSearchSpan] {
        self.rows.get(row).map_or(&[], Vec::as_slice)
    }

    /// The number of matches that intersected the visible line range.
    pub fn visited(&self) -> usize {
        self.visited
    }

    /// The number of painted spans in all rows.
    pub fn span_count(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }

    /// The number of painted spans that belong to the active match.
    pub fn active_span_count(&self) -> usize {
        self.rows
            .iter()
            .flat_map(|row| row.iter())
            .filter(|span| span.active)
            .count()
    }

    /// Whether no visible row has a match.
    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(Vec::is_empty)
    }
}

/// One accepted search result plus the active match index.
#[derive(Clone, Debug, Default)]
pub struct SearchOverlay {
    result: Option<Arc<SearchResult>>,
    active: Option<usize>,
}

impl SearchOverlay {
    /// Builds an overlay from a completed scan, clamping the active index.
    pub fn new(result: Arc<SearchResult>, active: Option<usize>) -> Self {
        let active = active.filter(|index| *index < result.match_count());
        Self {
            result: Some(result),
            active,
        }
    }

    /// Whether the overlay has no result.
    pub fn is_empty(&self) -> bool {
        self.result.is_none()
    }

    /// The complete scan result, if one is accepted.
    pub fn result(&self) -> Option<&Arc<SearchResult>> {
        self.result.as_ref()
    }

    /// The active match index.
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    /// The active match, when one is selected.
    pub fn active_match(&self) -> Option<SearchMatch> {
        let result = self.result.as_ref()?;
        self.active
            .and_then(|index| result.matches().get(index).copied())
    }

    /// The content version of the accepted result.
    pub fn content_version(&self) -> Option<u64> {
        self.result.as_ref().map(|result| result.content_version())
    }

    /// The scrollback generation of the accepted result.
    pub fn generation(&self) -> Option<u64> {
        self.result.as_ref().map(|result| result.generation())
    }

    /// Whether the result no longer describes the snapshot.
    ///
    /// A frame validates the version, generation and total row count before
    /// painting. Any mismatch means output, an eviction, a clear, an
    /// alternate-screen switch or a reflow changed the text after the scan.
    pub fn is_stale_for(&self, snapshot: &Snapshot) -> bool {
        let Some(result) = &self.result else {
            return false;
        };
        result.content_version() != snapshot.content_version()
            || result.generation() != snapshot.scrollback_generation()
            || result.total_lines()
                != snapshot
                    .history_size()
                    .saturating_add(snapshot.grid().screen_lines)
    }

    /// The ruler marks for the accepted matches, newest first bounded.
    pub fn marks(&self) -> Vec<OverviewSearchMark> {
        let Some(result) = &self.result else {
            return Vec::new();
        };
        let matches = result.matches();
        let first = matches.len().saturating_sub(MAX_RULER_MARKS);
        (first..matches.len())
            .map(|index| OverviewSearchMark {
                line: matches[index].start.line,
                active: self.active == Some(index),
            })
            .collect()
    }

    /// Clips the accepted matches to the snapshot's visible rows.
    pub fn visible(&self, snapshot: &Snapshot) -> VisibleSearch {
        let lines = snapshot.line_count();
        let rows = vec![Vec::new(); lines];
        let mut visible = VisibleSearch { rows, visited: 0 };
        let Some(result) = &self.result else {
            return visible;
        };
        let columns = snapshot.grid().columns;
        if lines == 0 || columns == 0 {
            return visible;
        }
        let history = snapshot.history_size() as i64;
        let offset = snapshot.display_offset() as i64;
        let first_line = history - offset;
        let last_line = first_line.saturating_add(lines as i64 - 1);
        let matches = result.matches();
        if matches.is_empty() {
            return visible;
        }
        // Matches are ordered and do not overlap, so both ends are sorted.
        let first = matches.partition_point(|matched| matched.end.line < first_line);
        let end = matches.partition_point(|matched| matched.start.line <= last_line);
        let last_column = columns - 1;
        for (offset_index, matched) in matches[first..end].iter().enumerate() {
            visible.visited += 1;
            let from = matched.start.line.max(first_line);
            let to = matched.end.line.min(last_line);
            if from > to {
                continue;
            }
            let active = self.active == Some(first + offset_index);
            for line in from..=to {
                let row = (line - first_line) as usize;
                let start = if line == matched.start.line {
                    matched.start.column
                } else {
                    0
                }
                .min(last_column);
                let end = if line == matched.end.line {
                    matched.end.column
                } else {
                    last_column
                }
                .min(last_column);
                if start > end {
                    continue;
                }
                if let Some(spans) = visible.rows.get_mut(row) {
                    spans.push(VisibleSearchSpan { start, end, active });
                }
            }
        }
        visible
    }

    /// The retained-history line the active match starts on.
    pub fn active_line(&self) -> Option<i64> {
        self.active_match().map(|matched| matched.start.line)
    }

    /// The retained-history starting point of a match, for preserving the
    /// selection across a rescan.
    pub fn active_start(&self) -> Option<SearchPosition> {
        self.active_match().map(|matched| matched.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_terminal::grid::{CellSize, GridSize};
    use tterm_terminal::search::{SearchCancellation, SearchOptions, SearchOutcome, SearchQuery};
    use tterm_terminal::terminal::{Config, Scroll, Terminal};

    fn model(lines: usize, columns: usize, screen: usize) -> Terminal {
        Terminal::new(
            Config {
                scrollback: lines,
                ..Config::default()
            },
            GridSize::new(columns, screen),
            CellSize::new(8, 16),
        )
    }

    fn scan(model: &Terminal, query: &str) -> Arc<SearchResult> {
        let query = SearchQuery::new(query, SearchOptions::default()).expect("the query compiles");
        let cancellation = SearchCancellation::new();
        match model.search_retained(&query, &cancellation) {
            SearchOutcome::Complete(result) => Arc::new(result),
            other => panic!("expected a complete scan, got {other:?}"),
        }
    }

    fn fill(model: &Terminal, lines: usize) {
        for line in 0..lines {
            model.write(format!("needle {line:03}\r\n").as_bytes());
        }
    }

    #[test]
    fn visible_spans_are_clipped_to_the_viewport_instead_of_the_whole_result() {
        let model = model(5_000, 20, 10);
        fill(&model, 2_000);
        let overlay = SearchOverlay::new(scan(&model, "needle"), None);
        let visible = overlay.visible(&model.snapshot());
        assert!(
            visible.visited() <= 10,
            "visited {} matches",
            visible.visited()
        );
        assert!(visible.span_count() <= 10);
    }

    #[test]
    fn a_match_that_wraps_into_the_viewport_is_painted_on_both_rows() {
        let model = model(30, 10, 6);
        // One long wrapped line: the match starts on the first physical row
        // and ends on the next.
        model.write(b"needle-needle-needle-needle\r\n");
        model.scroll(Scroll::Top);
        let overlay = SearchOverlay::new(scan(&model, "needle"), Some(0));
        let snapshot = model.snapshot();
        let visible = overlay.visible(&snapshot);
        assert!(visible.span_count() >= 2);
        assert!(visible.active_span_count() >= 1);
    }

    #[test]
    fn marks_keep_the_active_flag_and_are_bounded() {
        let model = model(MAX_RULER_MARKS + 100, 20, 5);
        fill(&model, MAX_RULER_MARKS + 50);
        let result = scan(&model, "needle");
        let total = result.match_count();
        assert!(total > MAX_RULER_MARKS);
        let overlay = SearchOverlay::new(Arc::clone(&result), Some(total - 1));
        let marks = overlay.marks();
        assert_eq!(marks.len(), MAX_RULER_MARKS);
        assert!(marks.last().is_some_and(|mark| mark.active));
        assert_eq!(marks.iter().filter(|mark| mark.active).count(), 1);
    }

    #[test]
    fn a_result_with_another_version_is_stale_for_the_snapshot() {
        let model = model(20, 20, 5);
        fill(&model, 10);
        let result = scan(&model, "needle");
        let overlay = SearchOverlay::new(result, Some(0));
        assert!(!overlay.is_stale_for(&model.snapshot()));
        model.write(b"more\r\n");
        assert!(overlay.is_stale_for(&model.snapshot()));
    }
}
