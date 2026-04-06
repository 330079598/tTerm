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
use tauri::{AppHandle, State};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

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

// SFTP 连接缓存
pub struct CachedSftpConnection {
    connection: ConnectedSftp,
    last_used: Instant,
}

// 连接缓存键
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct SftpConnectionKey {
    tab_id: String,
    host: String,
    port: u16,
    username: String,
}

// 全局 SFTP 连接池
pub type SftpConnectionPool = Arc<RwLock<HashMap<SftpConnectionKey, CachedSftpConnection>>>;

// 连接超时时间（5分钟无活动则关闭）
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

// 获取或创建 SFTP 连接
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

    // 清理过期连接
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

    // 检查是否已有可用连接
    if let Some(cached) = pool_guard.get_mut(&key) {
        cached.last_used = now;
        return Ok(());
    }

    // 创建新连接
    drop(pool_guard); // 释放写锁，避免阻塞其他操作
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

// 使用缓存的连接执行操作（使用宏简化）
macro_rules! with_sftp {
    ($app:expr, $tab_id:expr, $plan:expr, $prompts:expr, $pool:expr, $sftp:ident => $body:block) => {{
        let key = SftpConnectionKey {
            tab_id: $tab_id.to_string(),
            host: $plan.host.clone().ok_or("Host is required")?,
            port: $plan.port,
            username: $plan.username.clone().ok_or("Username is required")?,
        };

        // 确保连接存在
        get_or_create_sftp_connection($app, $tab_id, $plan, $prompts.clone(), $pool).await?;

        // 执行操作
        let pool_guard = $pool.read().await;
        let cached = pool_guard.get(&key).ok_or("Connection not found")?;
        let $sftp = &cached.connection.sftp;
        let result: Result<_, String> = async { $body }.await;

        // 如果操作失败，可能是连接断开，移除缓存
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
pub fn sftp_list_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: Option<String>,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<SftpDirectoryListing, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    runtime_state.runtime.block_on(async move {
        let requested_path = sanitize_target_path(path);
        
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            let current_path = match requested_path {
                Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
                None => sftp.canonicalize(".").await.map_err(map_sftp_error)?,
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
            Ok(SftpDirectoryListing {
                current_path: normalize_remote_path(&current_path),
                parent_path: parent_remote_path(&current_path),
                entries,
            })
        })
    })
}

#[tauri::command]
pub fn sftp_create_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    runtime_state.runtime.block_on(async move {
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            sftp.create_dir(path).await.map_err(map_sftp_error)
        })
    })
}

#[tauri::command]
pub fn sftp_delete_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    is_dir: bool,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    runtime_state.runtime.block_on(async move {
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            if is_dir {
                sftp.remove_dir(path).await.map_err(map_sftp_error)
            } else {
                sftp.remove_file(path).await.map_err(map_sftp_error)
            }
        })
    })
}

#[tauri::command]
pub fn sftp_rename_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    old_path: String,
    new_path: String,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    runtime_state.runtime.block_on(async move {
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            sftp.rename(old_path, new_path).await.map_err(map_sftp_error)
        })
    })
}

#[tauri::command]
pub fn sftp_upload_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_path: String,
    remote_path: String,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let data = std::fs::read(&local_path)
        .map_err(|err| format!("Failed to read local file '{local_path}': {err}"))?;

    runtime_state.runtime.block_on(async move {
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            let mut file = sftp.create(remote_path).await.map_err(map_sftp_error)?;
            file.write_all(&data)
                .await
                .map_err(|err| format!("Failed to write remote file: {err}"))?;
            file.shutdown()
                .await
                .map_err(|err| format!("Failed to finalize remote file: {err}"))?;
            Ok(())
        })
    })
}

#[tauri::command]
pub fn sftp_download_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    remote_path: String,
    local_path: String,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    runtime_state.runtime.block_on(async move {
        with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
            let data = sftp.read(remote_path).await.map_err(map_sftp_error)?;
            std::fs::write(&local_path, data)
                .map_err(|err| format!("Failed to write local file '{local_path}': {err}"))?;
            Ok(())
        })
    })
}
