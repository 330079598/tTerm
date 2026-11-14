//! Searching a folder and every folder under it by file name. The host's
//! `find` does the walking where it can; otherwise the walk runs over SFTP.
//! Matches stream to the drawer as they are found.

pub mod operations;
use crate::sftp::entry::SftpDirectoryEntry;
use crate::sftp::search::{SearchEvent, SearchResultSink};
use std::sync::Arc;
mod find;
pub(crate) mod matcher;
#[cfg(all(test, unix))]
mod real_sshd_tests;
mod walk;

use crate::sftp::internal::api::SftpHost;
use crate::sftp::internal::api::search::find::FindOutcome;
use crate::sftp::internal::api::search::find::FindSearch;
use crate::sftp::internal::api::search::matcher::NameMatcher;
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::types::SftpHandles;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// A search stops after this many matches: more would not be read, and
/// would slow the drawer down.
const MAX_RESULTS: usize = 1000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    root: String,
    query: String,
    regex: bool,
    /// Whether names starting with `.` are searched, and searched in.
    include_hidden: bool,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SearchMethod {
    Find,
    Sftp,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSummary {
    method: SearchMethod,
    found: usize,
    /// The search stopped at `MAX_RESULTS`.
    truncated: bool,
    cancelled: bool,
    skipped_folders: usize,
}

/// Where matches go, up to `MAX_RESULTS`.
pub struct ResultSink {
    pub channel: Arc<dyn SearchResultSink>,
    pub found: usize,
    pub truncated: bool,
}

impl ResultSink {
    fn room(&self) -> usize {
        MAX_RESULTS - self.found
    }

    /// Sends `entries`, cut at the limit. False once the limit cut them,
    /// when the search should stop.
    pub fn send(&mut self, mut entries: Vec<SftpDirectoryEntry>) -> bool {
        if entries.len() > self.room() {
            entries.truncate(self.room());
            self.truncated = true;
        }
        self.found += entries.len();
        if !entries.is_empty() {
            let _ = self.channel.send(SearchEvent::Matches { entries });
        }
        !self.truncated
    }

    pub fn progress(&self, scanned_folders: usize) {
        let _ = self.channel.send(SearchEvent::Progress { scanned_folders });
    }
}

async fn search(
    host: SftpHost<'_>,
    request: SearchRequest,
    channel: Arc<dyn SearchResultSink>,
    mut cancel: watch::Receiver<bool>,
) -> Result<SearchSummary, String> {
    let matcher = NameMatcher::new(&request.query, request.regex)?;
    let SftpHandles {
        sftp,
        ssh,
        launcher,
    } = host.handles().await?;
    let root = sftp
        .canonicalize(request.root)
        .await
        .map_err(map_sftp_error)?;
    let mut sink = ResultSink {
        channel,
        found: 0,
        truncated: false,
    };

    let find = FindSearch {
        launcher: &launcher,
        ssh: &ssh,
        sftp: &sftp,
        root: &root,
        matcher: &matcher,
        include_hidden: request.include_hidden,
    };
    let (method, cancelled, skipped_folders) = match find.run(&mut sink, &mut cancel).await {
        FindOutcome::Finished { cancelled } => (SearchMethod::Find, cancelled, 0),
        FindOutcome::Unavailable => {
            let walked = walk::walk(
                &sftp,
                &root,
                &matcher,
                request.include_hidden,
                &mut sink,
                &mut cancel,
            )
            .await?;
            (SearchMethod::Sftp, walked.cancelled, walked.skipped_folders)
        }
    };

    Ok(SearchSummary {
        method,
        found: sink.found,
        truncated: sink.truncated,
        cancelled,
        skipped_folders,
    })
}

#[cfg(test)]
mod container_tests;
