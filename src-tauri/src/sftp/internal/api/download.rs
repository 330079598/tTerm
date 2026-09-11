use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::transfer::{
    self, ProgressSink, RemoteChannels, TransferError, TransferOptions, TransferProgress,
};
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

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
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadBatchCompleteEvent {
    batch_id: String,
    cancelled: bool,
    error: Option<String>,
    transferred: u64,
    total: u64,
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
        progress_interval_bytes: 2 * 1024 * 1024,
    }
}

fn transfer_result_message(
    result: Result<transfer::TransferOutcome, TransferError>,
) -> Result<(), String> {
    match result {
        Ok(_) => Ok(()),
        Err(error) if error.is_cancelled() => Err("Download cancelled by user".to_string()),
        Err(error) => Err(error.message()),
    }
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

struct DirectoryDownloadItem {
    remote_path: String,
    relative_path: PathBuf,
    file_name: String,
    size: u64,
}

struct DirectoryDownloadPlan {
    directories: Vec<PathBuf>,
    files: Vec<DirectoryDownloadItem>,
    total_size: u64,
}

fn remote_basename(path: &str) -> Result<String, String> {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .map(ToString::to_string)
        .ok_or_else(|| format!("Failed to determine folder name for remote path '{path}'"))
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

async fn collect_directory_download_plan(
    sftp: &SftpSession,
    remote_root: &str,
    root_name: &str,
) -> Result<DirectoryDownloadPlan, String> {
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut total_size = 0u64;
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
                total_size = total_size.saturating_add(size);
                files.push(DirectoryDownloadItem {
                    remote_path,
                    relative_path,
                    file_name: name,
                    size,
                });
            }
        }
    }

    Ok(DirectoryDownloadPlan {
        directories,
        files,
        total_size,
    })
}

#[allow(clippy::too_many_arguments)]
async fn download_file_into_directory(
    app: &AppHandle,
    tab_id: &str,
    channels: RemoteChannels,
    cancel_map: &TransferCancelMap,
    batch_cancel_rx: Option<watch::Receiver<bool>>,
    batch_transfer_id: &str,
    item_transfer_id: &str,
    item: &DirectoryDownloadItem,
    local_path: &Path,
    total_size: u64,
    aggregate_base: u64,
) -> Result<(), String> {
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
                resumed_from: update.resumed_from,
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
    transfer_result_message(result)
}

async fn download_directory_with_progress(
    app: &AppHandle,
    tab_id: &str,
    sftp: &SftpSession,
    channels: &RemoteChannels,
    cancel_map: &TransferCancelMap,
    transfer_id: &str,
    remote_path: &str,
    local_parent_path: &str,
) -> Result<(), String> {
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx);

    let mut final_transferred = 0u64;
    let mut final_total = 0u64;

    let result = async {
        let root_name = remote_basename(remote_path)?;
        let plan = collect_directory_download_plan(sftp, remote_path, &root_name).await?;
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

            let item_result = download_file_into_directory(
                app,
                tab_id,
                channels.instance(),
                cancel_map,
                Some(cancel_rx.clone()),
                transfer_id,
                &item_transfer_id,
                item,
                &local_path,
                plan.total_size,
                aggregate_transferred,
            )
            .await;

            match item_result {
                Ok(()) => {
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
                        },
                    );
                    return Err(error);
                }
            }
        }
        final_transferred = aggregate_transferred;

        let root_local_path = local_parent.join(&root_name);
        let _ = app.emit(
            &format!("sftp-download-progress-{}", tab_id),
            DownloadProgressEvent {
                transfer_id: transfer_id.to_string(),
                local_path: root_local_path.display().to_string(),
                remote_path: remote_path.to_string(),
                transferred: aggregate_transferred,
                total: plan.total_size,
                progress: 100,
                resumed_from: 0,
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
        },
    );

    cancel_map.write().await.remove(transfer_id);
    result
}

#[tauri::command]
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

#[tauri::command]
pub async fn sftp_download_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_path: String,
    local_parent_path: String,
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
        &remote_path,
        &local_parent_path,
    )
    .await
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
