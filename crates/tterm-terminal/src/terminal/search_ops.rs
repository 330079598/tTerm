//! Search entry points on the terminal model.
//!
//! The matcher lives in [`crate::search`]. This module only holds the model
//! lock for the duration of one call.

use super::Terminal;
use crate::grid::{GridPoint, GridRange};
use crate::search::rows::read_rows;
use crate::search::{
    RetainedRows, SEARCH_CHUNK_ROWS, Search, SearchCancellation, SearchDirection, SearchOutcome,
    SearchQuery,
};

impl Terminal {
    /// The version of the model's current searchable text.
    ///
    /// It advances on every non-empty write, on a resize that changes the
    /// grid, and when a scrollback-capacity change drops history. Taking a
    /// snapshot or scrolling the viewport does not change it. A search result
    /// names the version it scanned; compare the two before using the result.
    /// The number is only comparable within this model, not across sessions.
    pub fn content_version(&self) -> u64 {
        self.state.lock().content_version
    }

    /// Finds the next match of `search` from `origin`.
    ///
    /// The match is computed while the terminal lock is held, so a scan that
    /// walks far into the scrollback delays the parser. P2-07 moves long
    /// searches to a worker that takes the lock in chunks; this call is the
    /// frame-sized version the first prototype needs.
    pub fn find_next(
        &self,
        search: &mut Search,
        origin: GridPoint,
        direction: SearchDirection,
    ) -> Option<GridRange> {
        let state = self.state.lock();
        search.find_in(&state.term, origin, direction)
    }

    /// Copies up to [`SEARCH_CHUNK_ROWS`] retained rows for a background scan.
    ///
    /// `start` is the retained-history line: zero is the oldest row the model
    /// holds, matching the overview marks. The lock is held only for this
    /// copy; the content version in the result lets a worker detect any write
    /// that changed text between chunks, while the scrollback generation
    /// still covers cache events such as eviction and an alternate-screen
    /// switch.
    pub fn retained_rows(&self, start: usize, max_rows: usize) -> RetainedRows {
        let state = self.state.lock();
        read_rows(
            state.term.grid(),
            state.scrollback_generation,
            state.content_version,
            start,
            max_rows.min(SEARCH_CHUNK_ROWS),
        )
    }

    /// Scans every retained row, releasing the model lock between chunks.
    ///
    /// The calling thread is expected to be a worker: the scan never holds the
    /// lock for the whole scrollback. `cancellation` stops it between rows and
    /// matches. A [`SearchOutcome::Stale`] result means the content changed
    /// during the scan and the caller should run it again. A caller that keeps
    /// the result can compare
    /// [`SearchResult::content_version`](crate::search::SearchResult::content_version)
    /// with [`Terminal::content_version`] to check it after the scan
    /// returned.
    pub fn search_retained(
        &self,
        query: &SearchQuery,
        cancellation: &SearchCancellation,
    ) -> SearchOutcome {
        crate::search::scan_chunks(query, cancellation, |start, max_rows| {
            self.retained_rows(start, max_rows)
        })
    }
}
