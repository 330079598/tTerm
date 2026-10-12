//! Downloading folders, and several remote paths as one transfer.

use super::{
    is_cancelled, next_transfer_id, transfer_options, transfer_outcome, DownloadBatchCompleteEvent,
    DownloadItemCompleteEvent, DownloadItemStartEvent, DownloadProgressEvent,
};
use crate::sftp::internal::api::conflict::{self, CandidateDecision, ConflictPolicy, SourceStat};
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::transfer::{self, ProgressSink, RemoteChannels, TransferProgress};
use crate::sftp::internal::types::TransferCancelMap;
use russh_sftp::client::SftpSession;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

/// Where a remote file should land locally under `policy`, or `None` to
/// skip it.
///
/// `skip_existing` is set by a directory-download retry, so files an earlier
/// attempt already finished are passed over instead of fetched again (see
/// [`conflict::decide_candidate`]).
async fn resolve_download_target(
    local_path: &Path,
    item: &DirectoryDownloadItem,
    policy: ConflictPolicy,
    skip_existing: bool,
) -> Result<Option<PathBuf>, String> {
    // The historical behaviour needs no stat at all.
    if policy == ConflictPolicy::Overwrite && !skip_existing {
        return Ok(Some(local_path.to_path_buf()));
    }

    let source = SourceStat {
        size: item.size,
        mtime: item.mtime,
    };
    let local_dir = local_path.parent().unwrap_or_else(|| Path::new(""));

    for attempt in 0..=conflict::MAX_RENAME_ATTEMPTS {
        let candidate = if attempt == 0 {
            local_path.to_path_buf()
        } else {
            local_dir.join(conflict::candidate_file_name(&item.file_name, attempt))
        };
        let target = conflict::local_target_stat(&candidate).await;
        match conflict::decide_candidate(policy, source, target, skip_existing) {
            CandidateDecision::Use => return Ok(Some(candidate)),
            CandidateDecision::Skip => return Ok(None),
            CandidateDecision::NextName => {}
        }
    }

    Err(format!(
        "No free name left for '{}' in '{}'",
        item.file_name,
        local_dir.display()
    ))
}

pub(super) struct DirectoryDownloadItem {
    pub(super) remote_path: String,
    pub(super) relative_path: PathBuf,
    pub(super) file_name: String,
    pub(super) size: u64,
    pub(super) mtime: Option<i64>,
}

pub(super) struct DirectoryDownloadPlan {
    pub(super) directories: Vec<PathBuf>,
    pub(super) files: Vec<DirectoryDownloadItem>,
    pub(super) total_size: u64,
}

pub(super) fn remote_basename(path: &str) -> Result<String, String> {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .map(ToString::to_string)
        .ok_or_else(|| format!("Failed to determine the name of remote path '{path}'"))
}

fn push_local_path_component(path: &mut PathBuf, component: &str) -> Result<(), String> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.contains('/')
        || component.contains('\\')
    {
        return Err(format!("Unsupported remote path component '{component}'"));
    }

    path.push(component);
    Ok(())
}

/// Plans one download of every remote root into the same local folder: a
/// folder brings its whole tree, a file just itself.
pub(super) async fn collect_download_plan(
    sftp: &SftpSession,
    remote_roots: &[String],
) -> Result<DirectoryDownloadPlan, String> {
    let mut plan = DirectoryDownloadPlan {
        directories: Vec::new(),
        files: Vec::new(),
        total_size: 0,
    };

    for remote_root in remote_roots {
        let root_name = remote_basename(remote_root)?;
        let metadata = sftp.metadata(remote_root).await.map_err(map_sftp_error)?;
        if metadata.is_dir() {
            collect_directory_download_plan(sftp, remote_root, &root_name, &mut plan).await?;
        } else {
            let size = metadata.size.unwrap_or(0);
            plan.total_size = plan.total_size.saturating_add(size);
            plan.files.push(DirectoryDownloadItem {
                remote_path: remote_root.clone(),
                relative_path: PathBuf::from(&root_name),
                file_name: root_name,
                size,
                mtime: metadata.mtime.map(|mtime| mtime as i64),
            });
        }
    }

    Ok(plan)
}

async fn collect_directory_download_plan(
    sftp: &SftpSession,
    remote_root: &str,
    root_name: &str,
    plan: &mut DirectoryDownloadPlan,
) -> Result<(), String> {
    let DirectoryDownloadPlan {
        directories,
        files,
        total_size,
    } = plan;
    let mut stack = vec![(remote_root.to_string(), PathBuf::from(root_name))];

    while let Some((remote_dir, relative_dir)) = stack.pop() {
        directories.push(relative_dir.clone());

        let entries = sftp.read_dir(&remote_dir).await.map_err(map_sftp_error)?;
        for entry in entries {
            let name = entry.file_name();
            let metadata = entry.metadata();
            let remote_path = crate::sftp::internal::paths::join_remote_path(&remote_dir, &name);
            let mut relative_path = relative_dir.clone();
            push_local_path_component(&mut relative_path, &name)?;

            if metadata.is_dir() {
                stack.push((remote_path, relative_path));
            } else {
                let size = metadata.size.unwrap_or(0);
                *total_size = total_size.saturating_add(size);
                files.push(DirectoryDownloadItem {
                    remote_path,
                    relative_path,
                    file_name: name,
                    size,
                    mtime: metadata.mtime.map(|mtime| mtime as i64),
                });
            }
        }
    }

    Ok(())
}

/// Where a download reports: the tab's events and the transfer cancel map.
#[derive(Clone, Copy)]
pub(super) struct DownloadTarget<'a> {
    pub app: &'a AppHandle,
    pub tab_id: &'a str,
    pub cancel_map: &'a TransferCancelMap,
}

/// What to download into a local directory, and how to treat files that
/// already exist there.
pub(super) struct DirectoryDownloadRequest<'a> {
    pub transfer_id: &'a str,
    pub remote_paths: &'a [String],
    pub local_parent_path: &'a str,
    pub skip_existing: Option<bool>,
    pub policy: ConflictPolicy,
}

/// Where one file of a batch sits in the batch's aggregate progress.
#[derive(Clone, Copy)]
struct BatchProgress<'a> {
    batch_transfer_id: &'a str,
    total_size: u64,
    aggregate_base: u64,
    /// Bytes already on disk before this item started (earlier skipped files
    /// plus resumed bytes of earlier files). Reported as part of the aggregate
    /// event's `resumed_from` so the UI measures throughput from the bytes
    /// this run actually fetched.
    existing_base: u64,
}

async fn download_file_into_directory(
    target: DownloadTarget<'_>,
    channels: RemoteChannels,
    batch_cancel_rx: Option<watch::Receiver<bool>>,
    batch: BatchProgress<'_>,
    item_transfer_id: &str,
    item: &DirectoryDownloadItem,
    local_path: &Path,
) -> Result<u64, String> {
    let DownloadTarget {
        app,
        tab_id,
        cancel_map,
    } = target;
    let BatchProgress {
        batch_transfer_id,
        total_size,
        aggregate_base,
        existing_base,
    } = batch;
    let (cancel_tx, cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(item_transfer_id.to_string(), cancel_tx.clone());

    let app_for_progress = app.clone();
    let tab_id_for_progress = tab_id.to_string();
    let item_transfer_id_for_progress = item_transfer_id.to_string();
    let batch_transfer_id_for_progress = batch_transfer_id.to_string();
    let local_path_for_progress = local_path.display().to_string();
    let remote_path_for_progress = item.remote_path.clone();
    let item_size = item.size;
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        if let Some(receiver) = batch_cancel_rx.as_ref() {
            if *receiver.borrow() {
                let _ = cancel_tx.send(true);
            }
        }

        let item_progress = if item_size > 0 {
            ((update.transferred as f64 / item_size as f64) * 100.0).min(100.0) as u32
        } else {
            100
        };
        let _ = app_for_progress.emit(
            &format!("sftp-download-progress-{}", tab_id_for_progress),
            DownloadProgressEvent {
                transfer_id: item_transfer_id_for_progress.clone(),
                local_path: local_path_for_progress.clone(),
                remote_path: remote_path_for_progress.clone(),
                transferred: update.transferred,
                total: item_size,
                progress: item_progress,
                resumed_from: update.resumed_from,
                parallelism: update.parallelism as u32,
                completed_chunks: update.completed_chunks,
                chunk_count: update.chunk_count,
            },
        );

        let aggregate = aggregate_base.saturating_add(update.transferred);
        let aggregate_progress = if total_size > 0 {
            ((aggregate as f64 / total_size as f64) * 100.0).min(100.0) as u32
        } else {
            0
        };
        let _ = app_for_progress.emit(
            &format!("sftp-download-progress-{}", tab_id_for_progress),
            DownloadProgressEvent {
                transfer_id: batch_transfer_id_for_progress.clone(),
                local_path: local_path_for_progress.clone(),
                remote_path: remote_path_for_progress.clone(),
                transferred: aggregate,
                total: total_size,
                progress: aggregate_progress,
                // Skipped bytes and this item's resumed bytes moved nothing in
                // this run; the UI derives the batch's throughput from
                // `transferred - resumed_from`.
                resumed_from: existing_base.saturating_add(update.resumed_from),
                parallelism: update.parallelism as u32,
                completed_chunks: update.completed_chunks,
                chunk_count: update.chunk_count,
            },
        );
    });

    let result = transfer::download_file(
        channels,
        local_path.to_path_buf(),
        item.remote_path.clone(),
        transfer_options(),
        cancel_rx,
        progress,
    )
    .await;

    cancel_map.write().await.remove(item_transfer_id);
    // The item's resumed bytes are handed back so the caller can keep the
    // aggregate speed honest: they were already on disk before this run.
    transfer_outcome(result).map(|outcome| outcome.resumed_from)
}

#[expect(
    clippy::too_many_lines,
    reason = "predates the 150-line limit; split it when it next changes"
)]
pub(super) async fn download_directory_with_progress(
    target: DownloadTarget<'_>,
    sftp: &SftpSession,
    channels: &RemoteChannels,
    request: DirectoryDownloadRequest<'_>,
) -> Result<(), String> {
    let DownloadTarget {
        app,
        tab_id,
        cancel_map,
    } = target;
    let DirectoryDownloadRequest {
        transfer_id,
        remote_paths,
        local_parent_path,
        skip_existing,
        policy,
    } = request;
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx);

    let mut final_transferred = 0u64;
    let mut final_total = 0u64;
    // Bytes of files this run skipped because the local copy was already
    // current: tracked across the loop so the batch-complete event can report
    // them even when the run is cancelled partway.
    let mut skipped_bytes = 0u64;
    // Bytes of files this run resumed from a previous partial attempt: like
    // the skipped ones they are part of the final size but moved no bytes.
    let mut resumed_bytes = 0u64;

    let result = async {
        let plan = collect_download_plan(sftp, remote_paths).await?;
        final_total = plan.total_size;
        let local_parent = PathBuf::from(local_parent_path);

        for directory in &plan.directories {
            if is_cancelled(&mut cancel_rx) {
                return Err("Download cancelled by user".to_string());
            }

            let local_dir = local_parent.join(directory);
            tokio::fs::create_dir_all(&local_dir).await.map_err(|err| {
                format!(
                    "Failed to create local directory '{}': {err}",
                    local_dir.display()
                )
            })?;
        }

        let mut aggregate_transferred = 0u64;
        for item in &plan.files {
            if is_cancelled(&mut cancel_rx) {
                return Err("Download cancelled by user".to_string());
            }

            let local_path = local_parent.join(&item.relative_path);
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|err| {
                    format!(
                        "Failed to create local directory '{}': {err}",
                        parent.display()
                    )
                })?;
            }

            let item_transfer_id = next_transfer_id();
            let resolved =
                resolve_download_target(&local_path, item, policy, skip_existing.unwrap_or(false))
                    .await;
            // The start event already carries the resolved name, so the
            // transfer list and a per-item retry both use the path the file
            // actually lands on.
            let local_path = match &resolved {
                Ok(Some(resolved_path)) => resolved_path.clone(),
                _ => local_path,
            };
            let local_path_string = local_path.display().to_string();
            let _ = app.emit(
                &format!("sftp-download-item-start-{}", tab_id),
                DownloadItemStartEvent {
                    transfer_id: item_transfer_id.clone(),
                    batch_id: transfer_id.to_string(),
                    file_name: item.file_name.clone(),
                    file_size: item.size,
                    local_path: local_path_string.clone(),
                    remote_path: item.remote_path.clone(),
                },
            );

            let resolved = match resolved {
                Ok(resolved) => resolved,
                Err(error) => {
                    let _ = app.emit(
                        &format!("sftp-download-item-complete-{}", tab_id),
                        DownloadItemCompleteEvent {
                            transfer_id: item_transfer_id,
                            error: Some(error.clone()),
                            local_path: local_path_string,
                            remote_path: item.remote_path.clone(),
                            cancelled: false,
                            success: false,
                            skipped: false,
                        },
                    );
                    return Err(error);
                }
            };

            // Skipped by the conflict policy, or by a retry because the local
            // file provably already holds the remote bytes.
            if resolved.is_none() {
                aggregate_transferred = aggregate_transferred.saturating_add(item.size);
                skipped_bytes = skipped_bytes.saturating_add(item.size);
                let _ = app.emit(
                    &format!("sftp-download-item-complete-{}", tab_id),
                    DownloadItemCompleteEvent {
                        transfer_id: item_transfer_id,
                        error: None,
                        local_path: local_path_string.clone(),
                        remote_path: item.remote_path.clone(),
                        cancelled: false,
                        success: true,
                        skipped: true,
                    },
                );
                let progress = if plan.total_size > 0 {
                    ((aggregate_transferred as f64 / plan.total_size as f64) * 100.0).min(100.0)
                        as u32
                } else {
                    0
                };
                let _ = app.emit(
                    &format!("sftp-download-progress-{}", tab_id),
                    DownloadProgressEvent {
                        transfer_id: transfer_id.to_string(),
                        local_path: local_path_string,
                        remote_path: item.remote_path.clone(),
                        transferred: aggregate_transferred,
                        total: plan.total_size,
                        progress,
                        // Counted in `transferred` for the progress bar, but no
                        // bytes moved for it in this run. Earlier resumed bytes
                        // are just as absent, hence the same sum as the
                        // aggregate events.
                        resumed_from: skipped_bytes.saturating_add(resumed_bytes),
                        parallelism: 0,
                        completed_chunks: 0,
                        chunk_count: 0,
                    },
                );
                continue;
            }

            let item_result = download_file_into_directory(
                target,
                channels.instance(),
                Some(cancel_rx.clone()),
                BatchProgress {
                    batch_transfer_id: transfer_id,
                    total_size: plan.total_size,
                    aggregate_base: aggregate_transferred,
                    existing_base: skipped_bytes.saturating_add(resumed_bytes),
                },
                &item_transfer_id,
                item,
                &local_path,
            )
            .await;

            match item_result {
                Ok(item_resumed) => {
                    resumed_bytes = resumed_bytes.saturating_add(item_resumed);
                    aggregate_transferred = aggregate_transferred.saturating_add(item.size);
                    let _ = app.emit(
                        &format!("sftp-download-item-complete-{}", tab_id),
                        DownloadItemCompleteEvent {
                            transfer_id: item_transfer_id,
                            error: None,
                            local_path: local_path_string,
                            remote_path: item.remote_path.clone(),
                            cancelled: false,
                            success: true,
                            skipped: false,
                        },
                    );
                }
                Err(error) => {
                    let cancelled = error.contains("cancelled");
                    let _ = app.emit(
                        &format!("sftp-download-item-complete-{}", tab_id),
                        DownloadItemCompleteEvent {
                            transfer_id: item_transfer_id,
                            error: if cancelled { None } else { Some(error.clone()) },
                            local_path: local_path_string,
                            remote_path: item.remote_path.clone(),
                            cancelled,
                            success: false,
                            skipped: false,
                        },
                    );
                    return Err(error);
                }
            }
        }
        final_transferred = aggregate_transferred;

        let (root_local_path, root_remote_path) = match remote_paths {
            [only] => (local_parent.join(remote_basename(only)?), only.clone()),
            _ => (local_parent.clone(), remote_paths.join(", ")),
        };
        let _ = app.emit(
            &format!("sftp-download-progress-{}", tab_id),
            DownloadProgressEvent {
                transfer_id: transfer_id.to_string(),
                local_path: root_local_path.display().to_string(),
                remote_path: root_remote_path,
                transferred: aggregate_transferred,
                total: plan.total_size,
                progress: 100,
                // Same accounting as the aggregate events: everything skipped
                // or resumed is already on disk, only the rest was fetched.
                resumed_from: skipped_bytes.saturating_add(resumed_bytes),
                parallelism: 0,
                completed_chunks: 0,
                chunk_count: 0,
            },
        );

        Ok(())
    }
    .await;

    let _ = app.emit(
        &format!("sftp-download-batch-complete-{}", tab_id),
        DownloadBatchCompleteEvent {
            batch_id: transfer_id.to_string(),
            cancelled: result
                .as_ref()
                .err()
                .map(|error| error.contains("cancelled"))
                .unwrap_or(false),
            error: result.as_ref().err().and_then(|error| {
                if error.contains("cancelled") {
                    None
                } else {
                    Some(error.clone())
                }
            }),
            transferred: final_transferred,
            total: final_total,
            skipped: skipped_bytes,
            resumed: resumed_bytes,
        },
    );

    cancel_map.write().await.remove(transfer_id);
    result
}

#[cfg(test)]
mod tests;
