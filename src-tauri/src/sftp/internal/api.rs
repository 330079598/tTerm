use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::store;
use crate::ssh::SecretStoreState;
use super::connection::{close_sftp, ensure_ssh_plan, get_or_create_sftp_connection, map_sftp_error};
use super::types::{
    SftpConnectionKey, SftpConnectionPool, SftpDirectoryEntry, SftpDirectoryListing,
    TransferCancelMap,
};
use russh_sftp::client::SftpSession;
use std::future::Future;
use std::pin::Pin;
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

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
    
    // Create cancel token for this transfer
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cancel_map.write().await.insert(transfer_id.clone(), cancel_tx);
    
    // Get file size
    let file_size = tokio::fs::metadata(&local_path)
        .await
        .map(|m| m.len())
        .map_err(|err| format!("Failed to get file metadata '{local_path}': {err}"))?;

    // Optimized: Use streaming read with larger chunks and buffering
    // Note: russh-sftp File doesn't support concurrent writes, so we use sequential writes
    // but with optimized chunk size and buffering for maximum throughput
    const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks for better throughput
    const READ_BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB read buffer
    const PROGRESS_UPDATE_BYTES: u64 = 2 * 1024 * 1024; // Update progress every 2MB
    
    let result = with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        use tokio::io::{AsyncReadExt, BufReader};
        
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(&remote_path).parent() {
            let parent_str = parent.to_string_lossy().to_string();
            if !parent_str.is_empty() && parent_str != "/" {
                // Try to create parent directory (ignore error if it already exists)
                let _ = sftp.create_dir(parent_str).await;
            }
        }
        
        // Open local file with buffered reader for better I/O performance
        let file = tokio::fs::File::open(&local_path)
            .await
            .map_err(|err| format!("Failed to open local file '{local_path}': {err}"))?;
        let mut local_file = BufReader::with_capacity(READ_BUFFER_SIZE, file);
        
        // Create remote file
        let mut remote_file = sftp.create(&remote_path).await
            .map_err(|err| format!("Failed to create remote file '{}': {}", remote_path, err))?;
        
        let mut total_written = 0u64;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut last_progress_update = 0u64;
        
        loop {
            // Check if cancelled
            if *cancel_rx.borrow_and_update() {
                return Err("Upload cancelled by user".to_string());
            }
            
            // Read chunk from local file
            let bytes_read = local_file.read(&mut buffer)
                .await
                .map_err(|err| format!("Failed to read local file: {err}"))?;
            
            if bytes_read == 0 {
                break; // EOF reached
            }
            
            // Write to remote file
            remote_file.write_all(&buffer[..bytes_read])
                .await
                .map_err(|err| format!("Failed to write remote file: {err}"))?;
            
            total_written += bytes_read as u64;
            
            // Update progress less frequently to reduce overhead
            if total_written - last_progress_update >= PROGRESS_UPDATE_BYTES 
                || total_written == file_size {
                last_progress_update = total_written;
                
                let progress = if file_size > 0 {
                    ((total_written as f64 / file_size as f64) * 100.0).min(100.0) as u32
                } else {
                    100
                };
                
                let _ = app.emit(&format!("sftp-upload-progress-{}", tab_id), serde_json::json!({
                    "localPath": local_path,
                    "transferred": total_written,
                    "total": file_size,
                    "progress": progress
                }));
            }
        }
        
        // Ensure final progress update
        if total_written > last_progress_update {
            let _ = app.emit(&format!("sftp-upload-progress-{}", tab_id), serde_json::json!({
                "localPath": local_path,
                "transferred": total_written,
                "total": file_size,
                "progress": 100
            }));
        }
        
        remote_file.shutdown()
            .await
            .map_err(|err| format!("Failed to finalize remote file: {err}"))?;
        
        Ok(())
    });
    
    // Clean up the cancel token
    cancel_map.write().await.remove(&transfer_id);
    
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
