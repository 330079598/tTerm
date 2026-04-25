use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh::Disconnect;
use russh_sftp::client::{error::Error as SftpError, SftpSession};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;

use crate::core::session::{
    normalize_connection, resolve_ssh_password, PtyConnectionOptions, SessionPlan,
};
use crate::core::state::{HostPromptMap, SessionKind};
use crate::sftp::internal::types::{
    CachedSftpConnection, ConnectedSftp, SftpConnectionKey, SftpConnectionPool,
};
use crate::ssh::{SecretStoreState, SshClientHandler, HOST_KEY_REJECTED_REASON};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300);

pub fn map_sftp_error(err: SftpError) -> String {
    err.to_string()
}

pub fn ensure_ssh_plan(
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

pub async fn connect_authenticated_ssh(
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
        profile_id: plan.profile_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.clone(),
        port: plan.port,
        prompts,
        user_rejected_host_key: host_key_rejected.clone(),
    };

    let mut session: client::Handle<SshClientHandler> = client::connect(
        std::sync::Arc::new(config),
        (host.as_str(), plan.port),
        handler,
    )
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

    let sftp = Arc::new(
        SftpSession::new(channel.into_stream())
            .await
            .map_err(map_sftp_error)?,
    );

    Ok(ConnectedSftp { ssh, sftp })
}

pub async fn close_sftp(connection: ConnectedSftp) {
    let ConnectedSftp { ssh, sftp } = connection;
    let _ = sftp.close().await;
    let _ = ssh
        .disconnect(Disconnect::ByApplication, "SFTP session closed", "en")
        .await;
}

pub async fn get_or_create_sftp_connection(
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

    let now = Instant::now();

    let mut pool_guard = pool.write().await;
    if let Some(cached) = pool_guard.get_mut(&key) {
        cached.last_used = now;
        return Ok(());
    }

    let expired_keys: Vec<_> = pool_guard
        .iter()
        .filter(|(_, cached)| now.duration_since(cached.last_used) > CONNECTION_TIMEOUT)
        .map(|(key, _)| key.clone())
        .collect();

    for expired_key in expired_keys {
        if let Some(cached) = pool_guard.remove(&expired_key) {
            tokio::spawn(async move {
                close_sftp(cached.connection).await;
            });
        }
    }

    drop(pool_guard);
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
