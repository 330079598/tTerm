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
    /// copy; the generation in the result lets a worker detect eviction, a
    /// clear, an alternate-screen switch or a resize between chunks.
    pub fn retained_rows(&self, start: usize, max_rows: usize) -> RetainedRows {
        let state = self.state.lock();
        read_rows(
            state.term.grid(),
            state.scrollback_generation,
            start,
            max_rows.min(SEARCH_CHUNK_ROWS),
        )
    }

    /// Scans every retained row, releasing the model lock between chunks.
    ///
    /// The calling thread is expected to be a worker: the scan never holds the
    /// lock for the whole scrollback. `cancellation` stops it between rows and
    /// matches. A [`SearchOutcome::Stale`] result means the scrollback
    /// generation changed during the scan and the caller should run it again.
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
