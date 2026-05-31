use crate::core::session::{PtyConnectionOptions, SessionPlan};
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::{
    SftpConnectionPool, SftpDirectoryEntry, SftpDirectoryListing, TransferCancelMap,
};
use crate::sftp::store;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::watch;

fn push_unique_candidate(candidates: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if !path.trim().is_empty() && !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn initial_directory_candidates(plan: &SessionPlan) -> Result<Vec<String>, String> {
    let host = plan.host.as_ref().ok_or("Host is required")?;
    let username = plan.username.as_ref().ok_or("Username is required")?;

    let mut candidates = Vec::new();
    if let Ok(Some(last_path)) = store::get_last_directory(host, plan.port, username) {
        push_unique_candidate(&mut candidates, last_path);
    }
    push_unique_candidate(&mut candidates, ".");
    push_unique_candidate(&mut candidates, "/");

    Ok(candidates)
}

async fn resolve_initial_directory(
    sftp: &SftpSession,
    plan: &SessionPlan,
) -> Result<String, String> {
    let mut last_error = None;

    for path in initial_directory_candidates(plan)? {
        match sftp.canonicalize(&path).await {
            Ok(canonical) => match sftp.read_dir(&canonical).await {
                Ok(_) => return Ok(canonical),
                Err(err) => last_error = Some(map_sftp_error(err)),
            },
            Err(err) => last_error = Some(map_sftp_error(err)),
        }
    }

    match last_error {
        Some(error) => Err(format!(
            "Failed to access any directory (last saved, home, or root): {error}"
        )),
        None => Err("Failed to access any directory (last saved, home, or root)".to_string()),
    }
}

async fn read_directory_entries(
    sftp: &SftpSession,
    current_path: &str,
) -> Result<Vec<SftpDirectoryEntry>, String> {
    let mut entries = sftp
        .read_dir(current_path)
        .await
        .map_err(map_sftp_error)?
        .map(|entry| {
            let name = entry.file_name();
            let metadata = entry.metadata();

            SftpDirectoryEntry {
                path: crate::sftp::internal::paths::join_remote_path(current_path, &name),
                name,
                is_dir: metadata.is_dir(),
                is_symlink: metadata.is_symlink(),
                size: metadata.size,
                modified_at: metadata.mtime.map(|value| value as i64 * 1000),
                permissions: metadata
                    .permissions
                    .map(|_| metadata.permissions().to_string()),
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

    crate::sftp::internal::paths::sort_entries(&mut entries);
    Ok(entries)
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

    let requested_path = crate::sftp::internal::paths::sanitize_target_path(path);

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        let current_path = match requested_path {
            Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
            None => resolve_initial_directory(sftp, &plan).await?,
        };

        let entries = read_directory_entries(sftp, &current_path).await?;

        // Save the current path as the last directory
        let host = plan.host.as_ref().ok_or("Host is required")?;
        let username = plan.username.as_ref().ok_or("Username is required")?;
        let normalized_path = crate::sftp::internal::paths::normalize_remote_path(&current_path);
        let _ = store::save_last_directory(host, plan.port, username, &normalized_path);

        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: crate::sftp::internal::paths::parent_remote_path(&current_path),
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgressEvent {
    transfer_id: String,
    local_path: String,
    remote_path: String,
    transferred: u64,
    total: u64,
    progress: u32,
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

async fn download_file_with_progress(
    app: &AppHandle,
    tab_id: &str,
    sftp: &russh_sftp::client::SftpSession,
    cancel_map: &TransferCancelMap,
    transfer_id: &str,
    remote_path: &str,
    local_path: &str,
) -> Result<(), String> {
    const CHUNK_SIZE: usize = 1024 * 1024;
    const PROGRESS_UPDATE_BYTES: u64 = 2 * 1024 * 1024;

    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cancel_map
        .write()
        .await
        .insert(transfer_id.to_string(), cancel_tx);

    let result = async {
        let mut remote_file = sftp.open(remote_path).await.map_err(map_sftp_error)?;
        let total = remote_file
            .metadata()
            .await
            .map_err(map_sftp_error)?
            .size
            .unwrap_or(0);

        let local_file = tokio::fs::File::create(local_path)
            .await
            .map_err(|err| format!("Failed to create local file '{local_path}': {err}"))?;
        let mut local_file = BufWriter::new(local_file);

        let mut total_read = 0u64;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut last_progress_update = 0u64;

        loop {
            if is_cancelled(&mut cancel_rx) {
                return Err("Download cancelled by user".to_string());
            }

            let bytes_read = remote_file
                .read(&mut buffer)
                .await
                .map_err(|err| format!("Failed to read remote file '{remote_path}': {err}"))?;

            if bytes_read == 0 {
                break;
            }

            if is_cancelled(&mut cancel_rx) {
                return Err("Download cancelled by user".to_string());
            }

            local_file
                .write_all(&buffer[..bytes_read])
                .await
                .map_err(|err| format!("Failed to write local file '{local_path}': {err}"))?;

            total_read += bytes_read as u64;

            if total_read - last_progress_update >= PROGRESS_UPDATE_BYTES
                || (total > 0 && total_read == total)
            {
                last_progress_update = total_read;
                let progress = if total > 0 {
                    ((total_read as f64 / total as f64) * 100.0).min(100.0) as u32
                } else {
                    0
                };

                let _ = app.emit(
                    &format!("sftp-download-progress-{}", tab_id),
                    DownloadProgressEvent {
                        transfer_id: transfer_id.to_string(),
                        local_path: local_path.to_string(),
                        remote_path: remote_path.to_string(),
                        transferred: total_read,
                        total,
                        progress,
                    },
                );
            }
        }

        if total_read > last_progress_update || total == 0 {
            let _ = app.emit(
                &format!("sftp-download-progress-{}", tab_id),
                DownloadProgressEvent {
                    transfer_id: transfer_id.to_string(),
                    local_path: local_path.to_string(),
                    remote_path: remote_path.to_string(),
                    transferred: total_read,
                    total,
                    progress: 100,
                },
            );
        }

        local_file
            .flush()
            .await
            .map_err(|err| format!("Failed to flush local file '{local_path}': {err}"))?;

        Ok(())
    }
    .await;

    cancel_map.write().await.remove(transfer_id);
    result
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

async fn download_file_into_directory(
    app: &AppHandle,
    tab_id: &str,
    sftp: &SftpSession,
    cancel_rx: &mut watch::Receiver<bool>,
    batch_transfer_id: &str,
    item_transfer_id: &str,
    item: &DirectoryDownloadItem,
    local_path: &Path,
    total_size: u64,
    aggregate_transferred: &mut u64,
) -> Result<(), String> {
    const CHUNK_SIZE: usize = 1024 * 1024;
    const PROGRESS_UPDATE_BYTES: u64 = 2 * 1024 * 1024;

    let mut remote_file = sftp.open(&item.remote_path).await.map_err(map_sftp_error)?;
    let local_file = tokio::fs::File::create(local_path).await.map_err(|err| {
        format!(
            "Failed to create local file '{}': {err}",
            local_path.display()
        )
    })?;
    let mut local_file = BufWriter::new(local_file);
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut last_progress_update = *aggregate_transferred;
    let mut item_transferred = 0u64;
    let mut last_item_progress_update = 0u64;

    loop {
        if is_cancelled(cancel_rx) {
            return Err("Download cancelled by user".to_string());
        }

        let bytes_read = remote_file.read(&mut buffer).await.map_err(|err| {
            format!("Failed to read remote file '{}': {err}", item.remote_path)
        })?;

        if bytes_read == 0 {
            break;
        }

        if is_cancelled(cancel_rx) {
            return Err("Download cancelled by user".to_string());
        }

        local_file.write_all(&buffer[..bytes_read]).await.map_err(|err| {
            format!("Failed to write local file '{}': {err}", local_path.display())
        })?;

        *aggregate_transferred = aggregate_transferred.saturating_add(bytes_read as u64);
        item_transferred = item_transferred.saturating_add(bytes_read as u64);

        if item_transferred - last_item_progress_update >= PROGRESS_UPDATE_BYTES
            || item_transferred == item.size
        {
            last_item_progress_update = item_transferred;
            let progress = if item.size > 0 {
                ((item_transferred as f64 / item.size as f64) * 100.0).min(100.0) as u32
            } else {
                100
            };

            let _ = app.emit(
                &format!("sftp-download-progress-{}", tab_id),
                DownloadProgressEvent {
                    transfer_id: item_transfer_id.to_string(),
                    local_path: local_path.display().to_string(),
                    remote_path: item.remote_path.clone(),
                    transferred: item_transferred,
                    total: item.size,
                    progress,
                },
            );
        }

        if *aggregate_transferred - last_progress_update >= PROGRESS_UPDATE_BYTES
            || (total_size > 0 && *aggregate_transferred == total_size)
        {
            last_progress_update = *aggregate_transferred;
            let progress = if total_size > 0 {
                ((*aggregate_transferred as f64 / total_size as f64) * 100.0).min(100.0) as u32
            } else {
                0
            };

            let _ = app.emit(
                &format!("sftp-download-progress-{}", tab_id),
                DownloadProgressEvent {
                    transfer_id: batch_transfer_id.to_string(),
                    local_path: local_path.display().to_string(),
                    remote_path: item.remote_path.clone(),
                    transferred: *aggregate_transferred,
                    total: total_size,
                    progress,
                },
            );
        }
    }

    local_file
        .flush()
        .await
        .map_err(|err| format!("Failed to flush local file '{}': {err}", local_path.display()))?;

    if item.size == 0 {
        let _ = app.emit(
            &format!("sftp-download-progress-{}", tab_id),
            DownloadProgressEvent {
                transfer_id: item_transfer_id.to_string(),
                local_path: local_path.display().to_string(),
                remote_path: item.remote_path.clone(),
                transferred: 0,
                total: 0,
                progress: 100,
            },
        );
    }

    Ok(())
}

async fn download_directory_with_progress(
    app: &AppHandle,
    tab_id: &str,
    sftp: &SftpSession,
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
                sftp,
                &mut cancel_rx,
                transfer_id,
                &item_transfer_id,
                item,
                &local_path,
                plan.total_size,
                &mut aggregate_transferred,
            )
            .await;

            match item_result {
                Ok(()) => {
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

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        download_file_with_progress(
            &app,
            &tab_id,
            sftp,
            cancel_map.inner(),
            &transfer_id,
            &remote_path,
            &local_path,
        )
        .await
    })
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

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        download_directory_with_progress(
            &app,
            &tab_id,
            sftp,
            cancel_map.inner(),
            &transfer_id,
            &remote_path,
            &local_parent_path,
        )
        .await
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
