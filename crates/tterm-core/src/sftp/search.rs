use crate::sftp::entry::SftpDirectoryEntry;
use serde::Serialize;

/// Incremental matches and directory counts from a recursive search.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SearchEvent {
    Matches {
        entries: Vec<SftpDirectoryEntry>,
    },
    /// Only the SFTP walk knows how far it got.
    Progress {
        scanned_folders: usize,
    },
}

/// Submits updates without waiting for the UI to consume them.
///
/// This interface has no acknowledgement or backpressure. The existing search
/// counts results even if the client transport returns an error.
pub trait SearchResultSink: Send + Sync {
    /// Submits one update without waiting for UI acknowledgement.
    ///
    /// Transport errors are returned to the caller; the existing search ignores
    /// them and continues counting matches without cancelling the search.
    fn send(&self, event: SearchEvent) -> Result<(), String>;
}
