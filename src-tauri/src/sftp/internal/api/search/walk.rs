//! Searching over SFTP alone, for hosts that cannot run `find`: Windows
//! servers, appliances with no shell, accounts limited to SFTP. Each folder
//! costs a round trip, so several are read at once.

use super::matcher::NameMatcher;
use super::ResultSink;
use crate::sftp::internal::api::base::directory_entry;
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::exec::cancelled;
use crate::sftp::internal::paths::join_remote_path;
use russh_sftp::client::fs::Metadata;
use russh_sftp::client::SftpSession;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinSet;

/// How many folders are read at once.
const WALK_CONCURRENCY: usize = 8;

/// How many folders are read between progress reports.
const PROGRESS_INTERVAL: usize = 25;

pub struct WalkOutcome {
    pub cancelled: bool,
    /// Folders that could not be read, such as ones without permission.
    pub skipped_folders: usize,
}

type FolderListing = (String, Result<Vec<(String, Metadata)>, String>);

async fn read_folder(sftp: Arc<SftpSession>, path: String) -> FolderListing {
    let entries = sftp
        .read_dir(path.clone())
        .await
        .map(|entries| {
            entries
                .map(|entry| (entry.file_name(), entry.metadata()))
                .collect()
        })
        .map_err(map_sftp_error);
    (path, entries)
}

/// Walks the tree under `root` breadth first. Symbolic links are not
/// followed, so a link back up the tree cannot loop. Fails only when `root`
/// itself cannot be read.
pub(super) async fn walk(
    sftp: &Arc<SftpSession>,
    root: &str,
    matcher: &NameMatcher,
    include_hidden: bool,
    sink: &mut ResultSink,
    cancel: &mut watch::Receiver<bool>,
) -> Result<WalkOutcome, String> {
    let mut outcome = WalkOutcome {
        cancelled: false,
        skipped_folders: 0,
    };
    let mut queue = VecDeque::from([root.to_string()]);
    let mut reads = JoinSet::new();
    let mut scanned = 0;
    loop {
        while reads.len() < WALK_CONCURRENCY {
            let Some(path) = queue.pop_front() else { break };
            reads.spawn(read_folder(sftp.clone(), path));
        }
        let joined = tokio::select! {
            () = cancelled(cancel) => {
                outcome.cancelled = true;
                return Ok(outcome);
            }
            joined = reads.join_next() => joined,
        };
        let Some(joined) = joined else {
            return Ok(outcome);
        };
        let (folder, listing) = joined.map_err(|err| format!("Search failed: {err}"))?;
        let listing = match listing {
            Ok(listing) => listing,
            Err(error) if scanned == 0 && folder == root => return Err(error),
            Err(_) => {
                outcome.skipped_folders += 1;
                continue;
            }
        };
        scanned += 1;

        let mut matches = Vec::new();
        for (name, metadata) in listing {
            if !include_hidden && name.starts_with('.') {
                continue;
            }
            if metadata.is_dir() {
                queue.push_back(join_remote_path(&folder, &name));
            }
            if matcher.matches(&name) {
                matches.push(directory_entry(&folder, name, &metadata));
            }
        }
        if !matches.is_empty() && !sink.send(matches) {
            return Ok(outcome);
        }
        if scanned % PROGRESS_INTERVAL == 0 {
            sink.progress(scanned);
        }
    }
}
