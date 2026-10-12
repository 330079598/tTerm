use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::conflict::{
    self, ConflictPolicy, ConflictReport, TransferConflict,
};
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::transfer::{
    self, ProgressSink, RemoteChannels, TransferError, TransferOptions, TransferProgress,
};
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::SecretStoreState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

mod directory;

use directory::{collect_download_plan, download_directory_with_progress, remote_basename};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    transfer_id: String,
    local_path: String,
    remote_path: String,
    transferred: u64,
    total: u64,
    progress: u32,
    resumed_from: u64,
    parallelism: u32,
    completed_chunks: u64,
    chunk_count: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadItemStartEvent {
    transfer_id: String,
    batch_id: String,
    file_name: String,
    file_size: u64,
    local_path: String,
    remote_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadItemCompleteEvent {
    transfer_id: String,
    error: Option<String>,
    local_path: String,
    remote_path: String,
    cancelled: bool,
    success: bool,
    /// The local file already held the remote bytes, so no bytes moved in
    /// this run. Reported so the UI does not derive a speed from them.
    skipped: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadBatchCompleteEvent {
    batch_id: String,
    cancelled: bool,
    error: Option<String>,
    transferred: u64,
    total: u64,
    /// Bytes of files the run skipped because the local copy was already
    /// current. They are included in `transferred` so the bar reaches 100%,
    /// but no bytes moved for them — reported separately so the UI does not
    /// derive a speed from them.
    skipped: u64,
    /// Bytes of files this run resumed from a previous partial attempt: they
    /// are part of `transferred` as well, but only the remainder was fetched.
    resumed: u64,
}

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

#[allow(clippy::too_many_arguments)]
async fn download_file_with_progress(
    app: &AppHandle,
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

    let app_for_progress = app.clone();
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
        let _ = app_for_progress.emit(
            &format!("sftp-download-progress-{}", tab_id_for_progress),
            DownloadProgressEvent {
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
        );
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
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
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
        &app,
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
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
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
        &app,
        &tab_id,
        &sftp,
        &channels,
        cancel_map.inner(),
        &transfer_id,
        &remote_paths,
        &local_parent_path,
        skip_existing,
        conflict_policy.unwrap_or_default(),
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
    prompt_state: State<'_, HostPromptMap>,
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
