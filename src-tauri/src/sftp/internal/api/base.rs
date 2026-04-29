use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::{
    SftpConnectionPool, SftpDirectoryEntry, SftpDirectoryListing, TransferCancelMap,
};
use crate::sftp::store;
use crate::ssh::SecretStoreState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::sync::watch;
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
                    path: crate::sftp::internal::paths::join_remote_path(&current_path, &name),
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

        crate::sftp::internal::paths::sort_entries(&mut entries);

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

fn is_cancelled(cancel_rx: &mut watch::Receiver<bool>) -> bool {
    *cancel_rx.borrow_and_update()
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
pub async fn get_file_size(local_path: String) -> Result<u64, String> {
    tokio::task::spawn_blocking(move || {
        std::fs::metadata(&local_path)
            .map(|metadata| metadata.len())
            .map_err(|err| format!("Failed to get file size for '{local_path}': {err}"))
    })
    .await
    .map_err(|err| format!("Task join error: {err}"))?
}
