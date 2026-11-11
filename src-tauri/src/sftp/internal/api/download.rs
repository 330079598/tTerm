use crate::event_sink::TauriEventSink;
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::{AppEvent, EventSink};
mod items;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::conflict::{self, ConflictReport, TransferConflict};
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::transfer::{
    self, ProgressSink, RemoteChannels, TransferError, TransferOptions, TransferProgress,
};
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::secret_store::types::SecretStoreState;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::watch;
use tterm_core::events::sftp::download::DownloadProgressEvent;
use tterm_core::sftp::conflict::ConflictPolicy;

mod directory;

use directory::{
    collect_download_plan, download_directory_with_progress, remote_basename,
    DirectoryDownloadRequest, DownloadTarget,
};

fn is_cancelled(cancel_rx: &mut watch::Receiver<bool>) -> bool {
    *cancel_rx.borrow_and_update()
}

fn next_transfer_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn transfer_options() -> TransferOptions {
    TransferOptions {
        parallelism: crate::sftp::internal::api::resolve_transfer_parallelism(),
        chunk_size: transfer::DEFAULT_CHUNK_SIZE,
        progress_interval: transfer::DEFAULT_PROGRESS_INTERVAL,
        pipeline_window: transfer::PIPELINE_WINDOW,
        rate_limiter: Some(crate::sftp::internal::api::transfer_rate_limiter(
            transfer::TransferDirection::Download,
        )),
    }
}

fn transfer_outcome(
    result: Result<transfer::TransferOutcome, TransferError>,
) -> Result<transfer::TransferOutcome, String> {
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) if error.is_cancelled() => Err("Download cancelled by user".to_string()),
        Err(error) => Err(error.message()),
    }
}

fn transfer_result_message(
    result: Result<transfer::TransferOutcome, TransferError>,
) -> Result<(), String> {
    transfer_outcome(result).map(|_| ())
}

/// Downloads one file through the transfer engine, reporting progress to
/// the tab and cancellable under `transfer_id`.
pub(crate) async fn download_file_with_progress(
    events: &Arc<dyn EventSink>,
    tab_id: &str,
    channels: RemoteChannels,
    cancel_map: &TransferCancelMap,
    transfer_id: &str,
    remote_path: &str,
    local_path: &str,
) -> Result<(), String> {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx);

    let events_for_progress = events.clone();
    let tab_id_for_progress = tab_id.to_string();
    let transfer_id_for_progress = transfer_id.to_string();
    let local_path_for_progress = local_path.to_string();
    let remote_path_for_progress = remote_path.to_string();
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        let progress = if update.total > 0 {
            ((update.transferred as f64 / update.total as f64) * 100.0).min(100.0) as u32
        } else {
            100
        };
        let _ = events_for_progress.emit(AppEvent::Sftp(SftpEvent::DownloadProgress {
            tab_id: tab_id_for_progress.to_string(),
            payload: DownloadProgressEvent {
                transfer_id: transfer_id_for_progress.clone(),
                local_path: local_path_for_progress.clone(),
                remote_path: remote_path_for_progress.clone(),
                transferred: update.transferred,
                total: update.total,
                progress,
                resumed_from: update.resumed_from,
                parallelism: update.parallelism as u32,
                completed_chunks: update.completed_chunks,
                chunk_count: update.chunk_count,
            },
        }));
    });

    let result = transfer::download_file(
        channels,
        PathBuf::from(local_path),
        remote_path.to_string(),
        transfer_options(),
        cancel_rx,
        progress,
    )
    .await;

    cancel_map.write().await.remove(transfer_id);
    transfer_result_message(result)
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_download_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_path: String,
    local_path: String,
    prompt_state: State<'_, std::sync::Arc<tterm_core::prompts::PromptBroker>>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let events: Arc<dyn EventSink> = Arc::new(TauriEventSink::new(app.clone()));
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let (_sftp, channels) = prepare_transfer(
        &app,
        &tab_id,
        &plan,
        prompt_state.inner().clone(),
        pool_state.inner(),
    )
    .await?;

    download_file_with_progress(
        &events,
        &tab_id,
        channels,
        cancel_map.inner(),
        &transfer_id,
        &remote_path,
        &local_path,
    )
    .await
}

/// Downloads remote files and folders, as one transfer, into
/// `local_parent_path`.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_download_paths(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_paths: Vec<String>,
    local_parent_path: String,
    skip_existing: Option<bool>,
    conflict_policy: Option<ConflictPolicy>,
    prompt_state: State<'_, std::sync::Arc<tterm_core::prompts::PromptBroker>>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let events: Arc<dyn EventSink> = Arc::new(TauriEventSink::new(app.clone()));
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let (sftp, channels) = prepare_transfer(
        &app,
        &tab_id,
        &plan,
        prompt_state.inner().clone(),
        pool_state.inner(),
    )
    .await?;

    download_directory_with_progress(
        DownloadTarget {
            events: &events,
            tab_id: &tab_id,
            cancel_map: cancel_map.inner(),
        },
        &sftp,
        &channels,
        DirectoryDownloadRequest {
            transfer_id: &transfer_id,
            remote_paths: &remote_paths,
            local_parent_path: &local_parent_path,
            skip_existing,
            policy: conflict_policy.unwrap_or_default(),
        },
    )
    .await
}

/// Pre-flight for `sftp_download_paths`: which of the files to download
/// already exist locally. Nothing can collide unless one of the remote roots
/// already exists locally, so the remote trees are only walked when one does.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_check_download_conflicts(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    remote_paths: Vec<String>,
    local_parent_path: String,
    prompt_state: State<'_, std::sync::Arc<tterm_core::prompts::PromptBroker>>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<ConflictReport, String> {
    let local_parent = PathBuf::from(&local_parent_path);
    let mut any_root_exists = false;
    for remote_path in &remote_paths {
        let root_name = remote_basename(remote_path)?;
        if tokio::fs::metadata(local_parent.join(&root_name))
            .await
            .is_ok()
        {
            any_root_exists = true;
            break;
        }
    }
    if !any_root_exists {
        return Ok(ConflictReport::default());
    }

    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        let download_plan = collect_download_plan(sftp, &remote_paths).await?;
        let mut report = ConflictReport {
            file_count: download_plan.files.len(),
            ..ConflictReport::default()
        };

        for item in &download_plan.files {
            let local_path = local_parent.join(&item.relative_path);
            let Some(target) = conflict::local_target_stat(&local_path).await else {
                continue;
            };
            report.push(TransferConflict {
                source_path: item.remote_path.clone(),
                target_path: local_path.display().to_string(),
                source_size: item.size,
                source_mtime: item.mtime,
                target_size: target.size,
                target_mtime: target.mtime,
                target_is_dir: target.is_dir,
            });
        }

        Ok(report)
    })
}

#[tauri::command]
pub async fn get_file_size(local_path: String) -> Result<u64, String> {
    tokio::task::spawn_blocking(move || {
        std::fs::metadata(&local_path)
            .map(|metadata| metadata.len())
            .map_err(|err| format!("Failed to get file size for '{local_path}': {err}"))
    })
    .await
    .map_err(|err| format!("Task join error: {err}"))?
}

#[cfg(test)]
mod contract_tests;
