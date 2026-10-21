//! Searching a folder and every folder under it by file name. The host's
//! `find` does the walking where it can; otherwise the walk runs over SFTP.
//! Matches stream to the drawer as they are found.

mod find;
mod matcher;
#[cfg(all(test, unix))]
mod real_sshd_tests;
mod walk;

use crate::core::session::types::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::{with_cancel, SftpHost};
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::types::{SftpConnectionPool, SftpDirectoryEntry, TransferCancelMap};
use crate::ssh::secret_store::types::SecretStoreState;
use find::{FindOutcome, FindSearch};
use matcher::NameMatcher;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
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

#[derive(Serialize)]
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
pub(super) struct ResultSink {
    channel: Channel<SearchEvent>,
    found: usize,
    truncated: bool,
}

impl ResultSink {
    fn room(&self) -> usize {
        MAX_RESULTS - self.found
    }

    /// Sends `entries`, cut at the limit. False once the limit cut them,
    /// when the search should stop.
    fn send(&mut self, mut entries: Vec<SftpDirectoryEntry>) -> bool {
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

    fn progress(&self, scanned_folders: usize) {
        let _ = self.channel.send(SearchEvent::Progress { scanned_folders });
    }
}

async fn search(
    host: SftpHost<'_>,
    request: SearchRequest,
    channel: Channel<SearchEvent>,
    mut cancel: watch::Receiver<bool>,
) -> Result<SearchSummary, String> {
    let matcher = NameMatcher::new(&request.query, request.regex)?;
    let (sftp, ssh) = host.handles().await?;
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

/// Searches `request.root` and the folders under it for names matching the
/// query, sending matches on `on_event` as they are found. Resolves once the
/// search ends; `sftp_cancel_remote_operation` with `search_id` stops it.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_search_entries(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    search_id: String,
    request: SearchRequest,
    on_event: Channel<SearchEvent>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<SearchSummary, String> {
    let host = SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: prompt_state.inner().clone(),
        secret_state: &secret_state,
        pool: pool_state.inner(),
    };
    with_cancel(cancel_map.inner(), &search_id, |cancel| {
        search(host, request, on_event, cancel)
    })
    .await
}
