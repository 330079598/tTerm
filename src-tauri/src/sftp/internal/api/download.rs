mod operations;
use operations::{
    sftp_check_download_conflicts_service, sftp_download_file_service, sftp_download_paths_service,
    SftpCheckDownloadConflictsDependencies, SftpCheckDownloadConflictsInput,
    SftpDownloadFileDependencies, SftpDownloadFileInput, SftpDownloadPathsDependencies,
    SftpDownloadPathsInput,
};
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::{AppEvent, EventSink};
mod items;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::conflict::ConflictReport;
use crate::sftp::internal::transfer::{
    self, ProgressSink, RemoteChannels, TransferError, TransferOptions, TransferProgress,
};
use crate::sftp::internal::types::TransferCancelMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::watch;
use tterm_core::events::sftp::download::DownloadProgressEvent;
use tterm_core::sftp::conflict::ConflictPolicy;

mod directory;

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
pub async fn sftp_download_file(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_path: String,
    local_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_download_file_service(
        SftpDownloadFileDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpDownloadFileInput {
            tab_id,
            connection,
            transfer_id,
            remote_path,
            local_path,
        },
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
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_paths: Vec<String>,
    local_parent_path: String,
    skip_existing: Option<bool>,
    conflict_policy: Option<ConflictPolicy>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_download_paths_service(
        SftpDownloadPathsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpDownloadPathsInput {
            tab_id,
            connection,
            transfer_id,
            remote_paths,
            local_parent_path,
            skip_existing,
            conflict_policy,
        },
    )
    .await
}

/// Pre-flight for `sftp_download_paths`: which of the files to download
/// already exist locally. Nothing can collide unless one of the remote roots
/// already exists locally, so the remote trees are only walked when one does.
#[tauri::command]
pub async fn sftp_check_download_conflicts(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    remote_paths: Vec<String>,
    local_parent_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ConflictReport, String> {
    sftp_check_download_conflicts_service(
        SftpCheckDownloadConflictsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpCheckDownloadConflictsInput {
            tab_id,
            connection,
            remote_paths,
            local_parent_path,
        },
    )
    .await
}

#[tauri::command]
pub async fn get_file_size(local_path: String) -> Result<u64, String> {
    operations::get_file_size_service(local_path).await
}

#[cfg(test)]
mod contract_tests;
