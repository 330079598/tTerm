use crate::sftp::entry::SftpDirectoryEntry;
use serde::Serialize;

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
