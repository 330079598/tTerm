mod store;

use crate::core::session::{
    normalize_connection, resolve_ssh_password, PtyConnectionOptions, SessionPlan,
};
use crate::core::state::{HostPromptMap, SessionKind};
use crate::ssh::{HOST_KEY_REJECTED_REASON, SshClientHandler};
use crate::ssh::SecretStoreState;
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh::Disconnect;
use russh_sftp::client::{error::Error as SftpError, SftpSession};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tokio::sync::watch;

// Cancel token for upload operations
pub type CancelSender = watch::Sender<bool>;

// Global map of transfer ID to cancel tokens
pub type TransferCancelMap = Arc<RwLock<HashMap<String, CancelSender>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub modified_at: Option<i64>,
    pub permissions: Option<String>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<SftpDirectoryEntry>,
}

struct ConnectedSftp {
    ssh: client::Handle<SshClientHandler>,
    sftp: SftpSession,
}

// SFTP connection cache
pub struct CachedSftpConnection {
    connection: ConnectedSftp,
    last_used: Instant,
}

// Connection cache key
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct SftpConnectionKey {
    tab_id: String,
    host: String,
    port: u16,
    username: String,
}

// Global SFTP connection pool
pub type SftpConnectionPool = Arc<RwLock<HashMap<SftpConnectionKey, CachedSftpConnection>>>;

// Connection timeout (close after 5 minutes of inactivity)
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300);

fn map_sftp_error(err: SftpError) -> String {
    err.to_string()
}

fn ensure_ssh_plan(
    app: &AppHandle,
    secret_state: &SecretStoreState,
    connection: Option<PtyConnectionOptions>,
) -> Result<SessionPlan, String> {
    let mut plan = normalize_connection(connection)?;
    if !matches!(plan.kind, SessionKind::Ssh) {
        return Err("SFTP requires an SSH connection".to_string());
    }
    resolve_ssh_password(app, secret_state, &mut plan)?;
    Ok(plan)
}

async fn connect_authenticated_ssh(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<client::Handle<SshClientHandler>, String> {
    let host = plan
        .host
        .clone()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .clone()
        .ok_or_else(|| "SSH username is required".to_string())?;

    let mut config = client::Config::default();
    config.keepalive_interval = Some(Duration::from_secs(plan.keepalive_interval_secs as u64));
    config.keepalive_max = plan.keepalive_count_max as usize;

    let host_key_rejected = Arc::new(AtomicBool::new(false));
    let handler = SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_name: plan.profile_name.clone(),
        host: host.clone(),
        port: plan.port,
        prompts,
        user_rejected_host_key: host_key_rejected.clone(),
    };

    let mut session: client::Handle<SshClientHandler> =
        client::connect(std::sync::Arc::new(config), (host.as_str(), plan.port), handler)
        .await
        .map_err(|err| {
            if host_key_rejected.load(AtomicOrdering::Relaxed) {
                HOST_KEY_REJECTED_REASON.to_string()
            } else {
                format!("SSH connect failed: {err}")
            }
        })?;

    let auth_result = if let Some(key_path) = &plan.private_key_path {
        let key_pair = russh::keys::load_secret_key(
            Path::new(key_path),
            plan.private_key_passphrase.as_deref(),
        )
        .map_err(|err| format!("Failed to load SSH key: {err}"))?;

        session
            .authenticate_publickey(
                username,
                PrivateKeyWithHashAlg::new(std::sync::Arc::new(key_pair), None),
            )
            .await
            .map_err(|err| format!("SSH key authentication failed: {err}"))?
    } else {
        let password = plan
            .password
            .clone()
            .ok_or_else(|| "SSH password is required".to_string())?;

        session
            .authenticate_password(username, password)
            .await
            .map_err(|err| format!("SSH authentication failed: {err}"))?
    };

    if !auth_result.success() {
        let _ = session
            .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
            .await;
        return Err("SSH authentication failed".to_string());
    }

    Ok(session)
}

async fn connect_sftp(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<ConnectedSftp, String> {
    let ssh: client::Handle<SshClientHandler> =
        connect_authenticated_ssh(app, tab_id, plan, prompts).await?;
    let channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|err| format!("Failed to start SFTP subsystem: {err}"))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(map_sftp_error)?;

    Ok(ConnectedSftp { ssh, sftp })
}

async fn close_sftp(connection: ConnectedSftp) {
    let ConnectedSftp { ssh, sftp } = connection;
    let _ = sftp.close().await;
    let _ = ssh
        .disconnect(Disconnect::ByApplication, "SFTP session closed", "en")
        .await;
}

// Get or create SFTP connection
async fn get_or_create_sftp_connection(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
) -> Result<(), String> {
    let key = SftpConnectionKey {
        tab_id: tab_id.to_string(),
        host: plan.host.clone().ok_or("Host is required")?,
        port: plan.port,
        username: plan.username.clone().ok_or("Username is required")?,
    };

    // Clean up expired connections
    let mut pool_guard = pool.write().await;
    let now = Instant::now();
    let expired_keys: Vec<_> = pool_guard
        .iter()
        .filter(|(_, cached)| now.duration_since(cached.last_used) > CONNECTION_TIMEOUT)
        .map(|(k, _)| k.clone())
        .collect();
    
    for expired_key in expired_keys {
        if let Some(cached) = pool_guard.remove(&expired_key) {
            tokio::spawn(async move {
                close_sftp(cached.connection).await;
            });
        }
    }

    // Check if there's an available connection
    if let Some(cached) = pool_guard.get_mut(&key) {
        cached.last_used = now;
        return Ok(());
    }

    // Create new connection
    drop(pool_guard); // Release write lock to avoid blocking other operations
    let connection = connect_sftp(app, tab_id, plan, prompts).await?;
    
    let mut pool_guard = pool.write().await;
    pool_guard.insert(
        key,
        CachedSftpConnection {
            connection,
            last_used: now,
        },
    );

    Ok(())
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

        // Execute operation
        let pool_guard = $pool.read().await;
        let cached = pool_guard.get(&key).ok_or("Connection not found")?;
        let $sftp = &cached.connection.sftp;
        let result: Result<_, String> = async { $body }.await;

        // If operation fails, connection may be broken, remove from cache
        if result.is_err() {
            drop(pool_guard);
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

fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        "/".to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn join_remote_path(parent: &str, name: &str) -> String {
    let parent = normalize_remote_path(parent);
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

fn parent_remote_path(path: &str) -> Option<String> {
    let path = normalize_remote_path(path);
    if path == "/" {
        return None;
    }

    match path.rsplit_once('/') {
        Some(("", _)) => Some("/".to_string()),
        Some((parent, _)) => Some(parent.to_string()),
        None => None,
    }
}

fn sort_entries(entries: &mut [SftpDirectoryEntry]) {
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
}

fn sanitize_target_path(path: Option<String>) -> Option<String> {
    path.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
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

    let requested_path = sanitize_target_path(path);
    
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
                    path: join_remote_path(&current_path, &name),
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

        sort_entries(&mut entries);
        
        // Save the current path as the last directory
        let host = plan.host.as_ref().ok_or("Host is required")?;
        let username = plan.username.as_ref().ok_or("Username is required")?;
        let normalized_path = normalize_remote_path(&current_path);
        let _ = store::save_last_directory(host, plan.port, username, &normalized_path);
        
        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: parent_remote_path(&current_path),
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
            sftp.remove_dir(path).await.map_err(map_sftp_error)
        } else {
            sftp.remove_file(path).await.map_err(map_sftp_error)
        }
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
