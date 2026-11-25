//! Session-facing control for the SSH reply ordering marker.

use crate::ordered_replies::{MARKER_QUERY, MarkerSource, OrderedReplies};

use super::Terminal;

impl Terminal {
    /// Enables the private marker the SSH session uses to order fixed replies.
    ///
    /// Call it before the first write. A local session keeps the marker
    /// disabled so its output is parsed once.
    pub fn enable_ordered_replies(&self) {
        let mut state = self.state.lock();
        if state.ordered_replies.is_none() {
            state.ordered_replies = Some(OrderedReplies::default());
        }
    }

    /// Writes the ordering marker for one fixed SSH reply.
    ///
    /// The marker is a normal unknown private mode query. Its provenance is
    /// recorded before parsing, so a program's own query with the same bytes
    /// still receives its real mode report.
    pub fn write_ordered_reply(&self, order: u64) {
        self.write_with_source(MARKER_QUERY, MarkerSource::Synthetic(order));
    }
}
