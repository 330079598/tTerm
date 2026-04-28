use crate::sftp::internal::types::TransferCancelMap;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
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

fn sanitize_local_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Local path is required".to_string());
    }

    Ok(PathBuf::from(trimmed))
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

async fn upload_single_file_with_progress(
    app: &AppHandle,
    tab_id: &str,
    sftp: &SftpSession,
    cancel_map: &TransferCancelMap,
    mut batch_cancel_rx: Option<watch::Receiver<bool>>,
    plan_item: &UploadFilePlanItem,
    transfer_id: &str,
) -> Result<(), String> {
    const CHUNK_SIZE: usize = 1024 * 1024;
    const READ_BUFFER_SIZE: usize = 4 * 1024 * 1024;
    const PROGRESS_UPDATE_BYTES: u64 = 2 * 1024 * 1024;

    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx);

    let result = async {
        if let Some(parent) = Path::new(&plan_item.remote_path).parent() {
            let parent_str = parent.to_string_lossy().to_string();
            if !parent_str.is_empty() {
                ensure_remote_dir_all(sftp, &parent_str, batch_cancel_rx.clone()).await?;
            }
        }

        let file = fs::File::open(&plan_item.local_path).await.map_err(|err| {
            format!(
                "Failed to open local file '{}': {err}",
                plan_item.local_path
            )
        })?;
        let mut local_file = BufReader::with_capacity(READ_BUFFER_SIZE, file);

        let mut remote_file = sftp.create(&plan_item.remote_path).await.map_err(|err| {
            format!(
                "Failed to create remote file '{}': {}",
                plan_item.remote_path, err
            )
        })?;

        let mut total_written = 0u64;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut last_progress_update = 0u64;

        loop {
            if is_cancelled(&mut cancel_rx)
                || batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false)
            {
                return Err("Upload cancelled by user".to_string());
            }

            let bytes_read = local_file.read(&mut buffer).await.map_err(|err| {
                format!(
                    "Failed to read local file '{}': {err}",
                    plan_item.local_path
                )
            })?;

            if bytes_read == 0 {
                break;
            }

            remote_file
                .write_all(&buffer[..bytes_read])
                .await
                .map_err(|err| {
                    format!(
                        "Failed to write remote file '{}': {err}",
                        plan_item.remote_path
                    )
                })?;

            total_written += bytes_read as u64;

            if total_written - last_progress_update >= PROGRESS_UPDATE_BYTES
                || total_written == plan_item.file_size
            {
                last_progress_update = total_written;
                let progress = if plan_item.file_size > 0 {
                    ((total_written as f64 / plan_item.file_size as f64) * 100.0).min(100.0) as u32
                } else {
                    100
                };

                let _ = app.emit(
                    &format!("sftp-upload-progress-{}", tab_id),
                    UploadItemProgressEvent {
                        transfer_id: transfer_id.to_string(),
                        local_path: plan_item.local_path.clone(),
                        transferred: total_written,
                        total: plan_item.file_size,
                        progress,
                    },
                );
            }
        }

        if total_written > last_progress_update {
            let _ = app.emit(
                &format!("sftp-upload-progress-{}", tab_id),
                UploadItemProgressEvent {
                    transfer_id: transfer_id.to_string(),
                    local_path: plan_item.local_path.clone(),
                    transferred: total_written,
                    total: plan_item.file_size,
                    progress: 100,
                },
            );
        }

        remote_file.shutdown().await.map_err(|err| {
            format!(
                "Failed to finalize remote file '{}': {err}",
                plan_item.remote_path
            )
        })?;

        Ok(())
    }
    .await;

    cancel_map.write().await.remove(transfer_id);
    result
}

pub mod commands;
