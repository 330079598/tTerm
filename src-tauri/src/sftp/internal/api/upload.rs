use crate::sftp::internal::transfer::{
    self, ProgressSink, RemoteChannels, TransferOptions, TransferProgress,
};
use crate::sftp::internal::types::TransferCancelMap;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::sync::watch;
#[derive(Clone)]
struct UploadFilePlanItem {
    file_name: String,
    file_size: u64,
    local_path: String,
    remote_path: String,
}

#[derive(Default)]
struct UploadPlan {
    directories: Vec<String>,
    files: Vec<UploadFilePlanItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadItemStartEvent {
    transfer_id: String,
    batch_id: Option<String>,
    file_name: String,
    file_size: u64,
    local_path: String,
    remote_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadBatchStartEvent {
    batch_id: String,
    display_name: String,
    local_path: String,
    /// Every path the batch was started with, so a retry can re-run exactly
    /// this batch instead of whatever batch happens to be the latest one.
    local_paths: Vec<String>,
    remote_base_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadBatchCompleteEvent {
    batch_id: String,
    cancelled: bool,
    error: Option<String>,
    failed: usize,
    succeeded: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadItemProgressEvent {
    transfer_id: String,
    local_path: String,
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
struct UploadItemCompleteEvent {
    transfer_id: String,
    error: Option<String>,
    local_path: String,
    remote_path: String,
    cancelled: bool,
    success: bool,
    /// The destination already held the source bytes, so no bytes moved in
    /// this run. Reported so the UI does not derive a speed from them.
    skipped: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadBatchResult {
    cancelled: bool,
    failed: usize,
    succeeded: usize,
}

struct UploadRootSummary {
    has_directories: bool,
    label: String,
    local_path: String,
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let s = [h1, h2];
                if let Ok(hex_str) = std::str::from_utf8(&s) {
                    if let Ok(byte) = u8::from_str_radix(hex_str, 16) {
                        bytes.push(byte);
                        continue;
                    }
                }
                bytes.push(b'%');
                bytes.push(h1);
                bytes.push(h2);
            } else {
                bytes.push(b'%');
                if let Some(h1) = h1 {
                    bytes.push(h1);
                }
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn sanitize_local_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Local path is required".to_string());
    }

    let stripped = if let Some(rest) = trimmed.strip_prefix("file://") {
        percent_decode(rest)
    } else {
        trimmed.to_string()
    };

    Ok(PathBuf::from(stripped))
}

fn file_name_from_path(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| format!("Failed to determine file name for '{}'", path.display()))
}

async fn collect_upload_plan(
    local_paths: &[String],
    remote_base_path: &str,
    mut cancel_rx: Option<watch::Receiver<bool>>,
) -> Result<UploadPlan, String> {
    let mut plan = UploadPlan::default();

    for local_path in local_paths {
        if cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
            return Err("Upload cancelled by user".to_string());
        }

        let root_path = sanitize_local_path(local_path)?;
        let metadata = fs::metadata(&root_path).await.map_err(|err| {
            format!(
                "Failed to read metadata for '{}': {err}",
                root_path.display()
            )
        })?;
        let root_name = file_name_from_path(&root_path)?;

        if metadata.is_dir() {
            let remote_root_path =
                crate::sftp::internal::paths::join_remote_path(remote_base_path, &root_name);
            collect_directory_upload_plan(
                &root_path,
                &remote_root_path,
                &mut plan,
                cancel_rx.clone(),
            )
            .await?;
        } else if metadata.is_file() {
            plan.files.push(UploadFilePlanItem {
                file_name: root_name.clone(),
                file_size: metadata.len(),
                local_path: root_path.to_string_lossy().into_owned(),
                remote_path: crate::sftp::internal::paths::join_remote_path(
                    remote_base_path,
                    &root_name,
                ),
            });
        } else {
            return Err(format!(
                "Unsupported upload item '{}': only files and folders are supported",
                root_path.display()
            ));
        }
    }

    Ok(plan)
}

fn collect_directory_upload_plan<'a>(
    current_path: &'a Path,
    current_remote_path: &'a str,
    plan: &'a mut UploadPlan,
    mut cancel_rx: Option<watch::Receiver<bool>>,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        if cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
            return Err("Upload cancelled by user".to_string());
        }

        if !plan
            .directories
            .iter()
            .any(|entry| entry == current_remote_path)
        {
            plan.directories.push(current_remote_path.to_string());
        }

        let mut entries = fs::read_dir(current_path).await.map_err(|err| {
            format!(
                "Failed to read directory '{}': {err}",
                current_path.display()
            )
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|err| {
            format!(
                "Failed to read directory entry '{}': {err}",
                current_path.display()
            )
        })? {
            if cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
                return Err("Upload cancelled by user".to_string());
            }

            let path = entry.path();
            let metadata = entry.metadata().await.map_err(|err| {
                format!("Failed to read metadata for '{}': {err}", path.display())
            })?;

            if metadata.is_dir() {
                let child_name = file_name_from_path(&path)?;
                let child_remote_path = crate::sftp::internal::paths::join_remote_path(
                    current_remote_path,
                    &child_name,
                );
                collect_directory_upload_plan(&path, &child_remote_path, plan, cancel_rx.clone())
                    .await?;
                continue;
            }

            if !metadata.is_file() {
                continue;
            }

            let file_name = file_name_from_path(&path)?;
            let remote_path =
                crate::sftp::internal::paths::join_remote_path(current_remote_path, &file_name);

            plan.files.push(UploadFilePlanItem {
                file_name,
                file_size: metadata.len(),
                local_path: path.to_string_lossy().into_owned(),
                remote_path,
            });
        }

        Ok(())
    })
}

async fn ensure_remote_dir_all(
    sftp: &SftpSession,
    remote_dir: &str,
    mut cancel_rx: Option<watch::Receiver<bool>>,
) -> Result<(), String> {
    let normalized = crate::sftp::internal::paths::normalize_remote_path(remote_dir);
    if normalized.is_empty() || normalized == "/" {
        return Ok(());
    }

    let mut current = if normalized.starts_with('/') {
        "/".to_string()
    } else {
        String::new()
    };

    for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
        if cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
            return Err("Upload cancelled by user".to_string());
        }

        current = crate::sftp::internal::paths::join_remote_path(&current, segment);
        match sftp.create_dir(&current).await {
            Ok(_) => {}
            Err(err) => {
                if sftp.read_dir(&current).await.is_err() {
                    return Err(format!(
                        "Failed to create remote directory '{}': {}",
                        current, err
                    ));
                }
            }
        }
    }

    Ok(())
}

fn next_transfer_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn is_cancelled(cancel_rx: &mut watch::Receiver<bool>) -> bool {
    *cancel_rx.borrow_and_update()
}

async fn inspect_upload_roots(local_paths: &[String]) -> Result<UploadRootSummary, String> {
    let mut has_directories = false;
    let mut label = String::new();
    let mut first_local_path = String::new();

    for (index, local_path) in local_paths.iter().enumerate() {
        let root_path = sanitize_local_path(local_path)?;
        let metadata = fs::metadata(&root_path).await.map_err(|err| {
            format!(
                "Failed to read metadata for '{}': {err}",
                root_path.display()
            )
        })?;

        if metadata.is_dir() {
            has_directories = true;
        }

        if index == 0 {
            label = file_name_from_path(&root_path)?;
            first_local_path = root_path.to_string_lossy().into_owned();
        }
    }

    if local_paths.len() > 1 {
        label = format!("{} items", local_paths.len());
    }

    Ok(UploadRootSummary {
        has_directories,
        label,
        local_path: first_local_path,
    })
}

#[allow(clippy::too_many_arguments)]
async fn upload_single_file_with_progress(
    app: &AppHandle,
    tab_id: &str,
    channels: RemoteChannels,
    cancel_map: &TransferCancelMap,
    batch_cancel_rx: Option<watch::Receiver<bool>>,
    plan_item: &UploadFilePlanItem,
    transfer_id: &str,
) -> Result<(), String> {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx.clone());

    let app_for_progress = app.clone();
    let tab_id_for_progress = tab_id.to_string();
    let transfer_id_for_progress = transfer_id.to_string();
    let local_path_for_progress = plan_item.local_path.clone();
    let progress: ProgressSink = Arc::new(move |update: TransferProgress| {
        if let Some(receiver) = batch_cancel_rx.as_ref() {
            if *receiver.borrow() {
                let _ = cancel_tx.send(true);
            }
        }

        let progress = if update.total > 0 {
            ((update.transferred as f64 / update.total as f64) * 100.0).min(100.0) as u32
        } else {
            100
        };

        let _ = app_for_progress.emit(
            &format!("sftp-upload-progress-{}", tab_id_for_progress),
            UploadItemProgressEvent {
                transfer_id: transfer_id_for_progress.clone(),
                local_path: local_path_for_progress.clone(),
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

    let options = TransferOptions {
        parallelism: crate::sftp::internal::api::resolve_transfer_parallelism(),
        chunk_size: transfer::DEFAULT_CHUNK_SIZE,
        progress_interval_bytes: transfer::DEFAULT_PROGRESS_INTERVAL_BYTES,
        pipeline_window: transfer::PIPELINE_WINDOW,
    };

    let result = transfer::upload_file(
        channels,
        PathBuf::from(&plan_item.local_path),
        plan_item.remote_path.clone(),
        options,
        cancel_rx,
        progress,
    )
    .await;

    cancel_map.write().await.remove(transfer_id);

    match result {
        Ok(_) => Ok(()),
        Err(error) if error.is_cancelled() => Err("Upload cancelled by user".to_string()),
        Err(error) => Err(error.message()),
    }
}

pub mod commands;
