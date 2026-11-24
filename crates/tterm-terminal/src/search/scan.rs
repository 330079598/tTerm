//! The pure scan over copied rows.

use crate::search::{
    RowStart, SearchCancellation, SearchOutcome, SearchQuery, SearchResult, SearchRow, SearchScan,
};

impl<'a> SearchScan<'a> {
    /// Starts a scan for one content version of the retained history.
    pub(crate) fn new(
        query: &'a SearchQuery,
        cancellation: &'a SearchCancellation,
        generation: u64,
        content_version: u64,
        total_lines: usize,
    ) -> Self {
        Self {
            query,
            cancellation,
            generation,
            content_version,
            total_lines,
            pending_units: Vec::new(),
            pending_positions: Vec::new(),
            pending_rows: Vec::new(),
            matches: Vec::new(),
            cancelled: false,
        }
    }

    /// Appends copied rows and scans every logical line they complete.
    ///
    /// A wrapped row stays pending until the row that ends the logical line
    /// arrives. If the last row is wrapped, [`SearchScan::finish`] scans the
    /// pending text as the end of the buffer.
    pub(crate) fn push_rows(&mut self, rows: &[SearchRow]) {
        for row in rows {
            if self.cancelled || self.cancellation.is_cancelled() {
                self.cancelled = true;
                return;
            }
            self.pending_rows.push(RowStart {
                unit: self.pending_units.len(),
                line: row.line,
            });
            self.pending_units.extend_from_slice(&row.units);
            self.pending_positions.extend_from_slice(&row.positions);
            if !row.wrapped {
                self.flush();
            }
        }
    }

    /// Consumes the scan and returns its result, or the reason it stopped.
    pub(crate) fn finish(mut self) -> SearchOutcome {
        if self.cancelled || self.cancellation.is_cancelled() {
            return SearchOutcome::Cancelled;
        }
        if !self.pending_units.is_empty() {
            self.flush();
            if self.cancelled {
                return SearchOutcome::Cancelled;
            }
        }
        SearchOutcome::Complete(SearchResult::new(
            self.generation,
            self.content_version,
            self.total_lines,
            self.matches,
        ))
    }

    fn flush(&mut self) {
        if !self.pending_units.is_empty() {
            let accepted = self.query.matches_in(
                &self.pending_units,
                &self.pending_positions,
                &self.pending_rows,
                &mut self.matches,
                self.cancellation,
            );
            if !accepted {
                self.cancelled = true;
            }
        }
        self.pending_units.clear();
        self.pending_positions.clear();
        self.pending_rows.clear();
    }
}
