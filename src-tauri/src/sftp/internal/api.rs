use super::connection::{
    close_sftp, ensure_ssh_plan, get_or_create_sftp_connection, map_sftp_error,
};
use super::types::{
    SftpConnectionKey, SftpConnectionPool, SftpDirectoryEntry, SftpDirectoryListing,
    TransferCancelMap,
};
use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::store;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tauri::{AppHandle, Emitter, State};
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
            let remote_root_path = super::paths::join_remote_path(remote_base_path, &root_name);
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
                remote_path: super::paths::join_remote_path(remote_base_path, &root_name),
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
                let child_remote_path =
                    super::paths::join_remote_path(current_remote_path, &child_name);
                collect_directory_upload_plan(&path, &child_remote_path, plan, cancel_rx.clone())
                    .await?;
                continue;
            }

            if !metadata.is_file() {
                continue;
            }

            let file_name = file_name_from_path(&path)?;
            let remote_path = super::paths::join_remote_path(current_remote_path, &file_name);

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
    let normalized = super::paths::normalize_remote_path(remote_dir);
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

        current = super::paths::join_remote_path(&current, segment);
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

// Execute operations using cached connection (simplified with macro)
macro_rules! with_sftp {
    ($app:expr, $tab_id:expr, $plan:expr, $prompts:expr, $pool:expr, $sftp:ident => $body:block) => {{
        let key = SftpConnectionKey {
            tab_id: $tab_id.to_string(),
            host: $plan.host.clone().ok_or("Host is required")?,
            port: $plan.port,
            username: $plan.username.clone().ok_or("Username is required")?,
        };

        // Ensure connection exists
        get_or_create_sftp_connection($app, $tab_id, $plan, $prompts.clone(), $pool).await?;

        // Clone the session handle so long-running operations do not hold the pool lock.
        let sftp = {
            let pool_guard = $pool.read().await;
            let cached = pool_guard.get(&key).ok_or("Connection not found")?;
            cached.connection.sftp.clone()
        };

        let result: Result<_, String> = async {
            let $sftp = sftp.as_ref();
            $body
        }
        .await;

        // If operation fails, connection may be broken, remove from cache.
        if result.is_err() {
            let mut pool_guard = $pool.write().await;
            if let Some(cached) = pool_guard.remove(&key) {
                tokio::spawn(async move {
                    close_sftp(cached.connection).await;
                });
            }
        }

        result
    }};
}

#[tauri::command]
pub async fn sftp_list_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: Option<String>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<SftpDirectoryListing, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let requested_path = super::paths::sanitize_target_path(path);

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        let current_path = match requested_path {
            Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
            None => {
                // Try to get last directory from store
                let host = plan.host.as_ref().ok_or("Host is required")?;
                let username = plan.username.as_ref().ok_or("Username is required")?;

                // Try multiple fallback paths in order:
                // 1. Last saved directory
                // 2. User home directory (.)
                // 3. Root directory (/)
                let fallback_paths = match store::get_last_directory(host, plan.port, username) {
                    Ok(Some(last_path)) => vec![last_path, ".".to_string(), "/".to_string()],
                    _ => vec![".".to_string(), "/".to_string()],
                };

                let mut resolved_path = None;
                for path in fallback_paths {
                    if let Ok(canonical) = sftp.canonicalize(&path).await {
                        resolved_path = Some(canonical);
                        break;
                    }
                }

                resolved_path.ok_or("Failed to access any directory (home, root, or last saved)")?
            }
        };

        let mut entries = sftp
            .read_dir(&current_path)
            .await
            .map_err(map_sftp_error)?
            .map(|entry| {
                let name = entry.file_name();
                let metadata = entry.metadata();

                SftpDirectoryEntry {
                    path: super::paths::join_remote_path(&current_path, &name),
                    name,
                    is_dir: metadata.is_dir(),
                    is_symlink: metadata.is_symlink(),
                    size: metadata.size,
                    modified_at: metadata.mtime.map(|value| value as i64 * 1000),
                    permissions: metadata.permissions.map(|_| metadata.permissions().to_string()),
                    owner: metadata
                        .user
                        .clone()
                        .or_else(|| metadata.uid.map(|value| value.to_string())),
                    group: metadata
                        .group
                        .clone()
                        .or_else(|| metadata.gid.map(|value| value.to_string())),
                }
            })
            .collect::<Vec<_>>();

        super::paths::sort_entries(&mut entries);

        // Save the current path as the last directory
        let host = plan.host.as_ref().ok_or("Host is required")?;
        let username = plan.username.as_ref().ok_or("Username is required")?;
        let normalized_path = super::paths::normalize_remote_path(&current_path);
        let _ = store::save_last_directory(host, plan.port, username, &normalized_path);

        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: super::paths::parent_remote_path(&current_path),
            entries,
        })
    })
}

#[tauri::command]
pub async fn sftp_create_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        sftp.create_dir(path).await.map_err(map_sftp_error)
    })
}

#[tauri::command]
pub async fn sftp_delete_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    is_dir: bool,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        if is_dir {
            delete_directory_recursive(sftp, path).await
        } else {
            sftp.remove_file(path).await.map_err(map_sftp_error)
        }
    })
}

fn delete_directory_recursive<'a>(
    sftp: &'a SftpSession,
    path: String,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        // List all entries in the directory
        let entries = sftp.read_dir(&path).await.map_err(map_sftp_error)?;

        // Delete each entry recursively
        for entry in entries {
            let name = entry.file_name();
            let entry_path = if path.ends_with('/') {
                format!("{}{}", path, name)
            } else {
                format!("{}/{}", path, name)
            };

            if entry.metadata().is_dir() {
                // Recursively delete subdirectory
                delete_directory_recursive(sftp, entry_path).await?;
            } else {
                // Delete file
                sftp.remove_file(entry_path).await.map_err(map_sftp_error)?;
            }
        }

        // Finally, delete the now-empty directory
        sftp.remove_dir(path).await.map_err(map_sftp_error)
    })
}

#[tauri::command]
pub async fn sftp_rename_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    old_path: String,
    new_path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        sftp.rename(old_path, new_path).await.map_err(map_sftp_error)
    })
}

#[tauri::command]
pub async fn sftp_upload_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_path: String,
    remote_path: String,
    transfer_id: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let metadata = fs::metadata(&local_path)
        .await
        .map_err(|err| format!("Failed to get file metadata '{local_path}': {err}"))?;
    let file_name = file_name_from_path(Path::new(&local_path))?;
    let plan_item = UploadFilePlanItem {
        file_name,
        file_size: metadata.len(),
        local_path,
        remote_path,
    };

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        upload_single_file_with_progress(
            &app,
            &tab_id,
            sftp,
            cancel_map.inner(),
            None,
            &plan_item,
            &transfer_id,
        ).await
    })
}

#[tauri::command]
pub async fn sftp_upload_paths(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<UploadBatchResult, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let root_summary = inspect_upload_roots(&local_paths).await?;
    let batch_id = next_transfer_id();

    let mut batch_cancel_rx = None;
    if root_summary.has_directories {
        let (batch_cancel_tx, next_batch_cancel_rx) = watch::channel(false);
        cancel_map
            .write()
            .await
            .insert(batch_id.clone(), batch_cancel_tx);
        batch_cancel_rx = Some(next_batch_cancel_rx);

        let _ = app.emit(
            &format!("sftp-upload-batch-start-{}", tab_id),
            UploadBatchStartEvent {
                batch_id: batch_id.clone(),
                display_name: root_summary.label.clone(),
                local_path: root_summary.local_path.clone(),
                remote_base_path: remote_base_path.clone(),
            },
        );
    }

    let upload_plan =
        match collect_upload_plan(&local_paths, &remote_base_path, batch_cancel_rx.clone()).await {
            Ok(plan) => plan,
            Err(error) => {
                if root_summary.has_directories {
                    let cancelled = error.contains("cancelled");
                    let _ = app.emit(
                        &format!("sftp-upload-batch-complete-{}", tab_id),
                        UploadBatchCompleteEvent {
                            batch_id: batch_id.clone(),
                            cancelled,
                            error: if cancelled { None } else { Some(error.clone()) },
                            failed: 0,
                            succeeded: 0,
                        },
                    );
                    cancel_map.write().await.remove(&batch_id);

                    if cancelled {
                        return Ok(UploadBatchResult {
                            cancelled: true,
                            failed: 0,
                            succeeded: 0,
                        });
                    }
                }

                return Err(error);
            }
        };

    let result = with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        let cancel_map = cancel_map.inner().clone();
        let app = app.clone();
        let tab_id = tab_id.clone();
        let batch_id = batch_id.clone();
        let directories = upload_plan.directories.clone();
        let files = upload_plan.files.clone();
        let mut batch_cancel_rx = batch_cancel_rx.clone();
        let batch_enabled = root_summary.has_directories;

        async move {
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            let mut cancelled = false;

            for directory in directories {
                if batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
                    cancelled = true;
                    break;
                }

                match ensure_remote_dir_all(sftp, &directory, batch_cancel_rx.clone()).await {
                    Ok(()) => {}
                    Err(error) => {
                        if error.contains("cancelled") {
                            cancelled = true;
                            break;
                        }
                        return Err(error);
                    }
                }
            }

            if !cancelled {
                for plan_item in files {
                    if batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
                        cancelled = true;
                        break;
                    }

                    let transfer_id = next_transfer_id();
                    let _ = app.emit(
                        &format!("sftp-upload-item-start-{}", tab_id),
                        UploadItemStartEvent {
                            transfer_id: transfer_id.clone(),
                            batch_id: if batch_enabled {
                                Some(batch_id.clone())
                            } else {
                                None
                            },
                            file_name: plan_item.file_name.clone(),
                            file_size: plan_item.file_size,
                            local_path: plan_item.local_path.clone(),
                            remote_path: plan_item.remote_path.clone(),
                        },
                    );

                    let result = upload_single_file_with_progress(
                        &app,
                        &tab_id,
                        sftp,
                        &cancel_map,
                        batch_cancel_rx.clone(),
                        &plan_item,
                        &transfer_id,
                    )
                    .await;

                    match result {
                        Ok(()) => {
                            succeeded += 1;
                            let _ = app.emit(
                                &format!("sftp-upload-item-complete-{}", tab_id),
                                UploadItemCompleteEvent {
                                    transfer_id,
                                    error: None,
                                    local_path: plan_item.local_path,
                                    remote_path: plan_item.remote_path,
                                    cancelled: false,
                                    success: true,
                                },
                            );
                        }
                        Err(error) => {
                            let item_cancelled = error.contains("cancelled");
                            if item_cancelled {
                                cancelled = true;
                            } else {
                                failed += 1;
                            }

                            let _ = app.emit(
                                &format!("sftp-upload-item-complete-{}", tab_id),
                                UploadItemCompleteEvent {
                                    transfer_id,
                                    error: Some(error),
                                    local_path: plan_item.local_path,
                                    remote_path: plan_item.remote_path,
                                    cancelled: item_cancelled,
                                    success: false,
                                },
                            );

                            if item_cancelled {
                                break;
                            }
                        }
                    }
                }
            }

            Ok(UploadBatchResult {
                cancelled,
                failed,
                succeeded,
            })
        }
        .await
    });

    if root_summary.has_directories {
        let event = match &result {
            Ok(batch_result) => UploadBatchCompleteEvent {
                batch_id: batch_id.clone(),
                cancelled: batch_result.cancelled,
                error: None,
                failed: batch_result.failed,
                succeeded: batch_result.succeeded,
            },
            Err(error) => UploadBatchCompleteEvent {
                batch_id: batch_id.clone(),
                cancelled: false,
                error: Some(error.clone()),
                failed: 0,
                succeeded: 0,
            },
        };

        let _ = app.emit(&format!("sftp-upload-batch-complete-{}", tab_id), event);
        cancel_map.write().await.remove(&batch_id);
    }

    result
}

#[tauri::command]
pub async fn sftp_cancel_upload(
    transfer_id: String,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let map = cancel_map.read().await;
    if let Some(sender) = map.get(&transfer_id) {
        let _ = sender.send(true);
        Ok(())
    } else {
        Err("Transfer not found or already completed".to_string())
    }
}

#[tauri::command]
pub async fn sftp_download_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    remote_path: String,
    local_path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let data = with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        sftp.read(remote_path).await.map_err(map_sftp_error)
    })?;

    // Write file in background thread to avoid blocking
    tokio::task::spawn_blocking(move || {
        std::fs::write(&local_path, data)
            .map_err(|err| format!("Failed to write local file '{local_path}': {err}"))
    })
    .await
    .map_err(|err| format!("Task join error: {err}"))??;

    Ok(())
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
