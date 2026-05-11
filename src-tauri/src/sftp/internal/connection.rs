use russh::client;
use russh::Disconnect;
use russh_sftp::client::{error::Error as SftpError, SftpSession};
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
use crate::ssh::{
    open_target_ssh_session, JumpChain, SecretStoreState, SshClientHandler,
};
use crate::ssh::ConnectionStatusOptions;

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

/// Open an authenticated SSH session for SFTP use, routing through a jump
/// host when the plan includes one.  Returns both the target session and an
/// optional jump session that must be kept alive alongside it.
pub async fn connect_authenticated_ssh(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<(Option<JumpChain>, client::Handle<SshClientHandler>), String> {
    let host = plan
        .host
        .clone()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .clone()
        .ok_or_else(|| "SSH username is required".to_string())?;

    open_target_ssh_session(
        app,
        tab_id,
        plan.profile_id.as_deref(),
        &plan.profile_name,
        &host,
        plan.port,
        &username,
        plan.private_key_path.as_deref(),
        plan.private_key_passphrase.as_deref(),
        plan.password.as_deref(),
        plan.keepalive_interval_secs,
        plan.keepalive_count_max,
        &plan.jump_hosts,
        prompts,
        ConnectionStatusOptions::SILENT,
    )
    .await
}

async fn connect_sftp(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<ConnectedSftp, String> {
    let (jump_chain, ssh) = connect_authenticated_ssh(app, tab_id, plan, prompts).await?;

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

    Ok(ConnectedSftp {
        jump_chain,
        ssh,
        sftp,
    })
}

pub async fn close_sftp(connection: ConnectedSftp) {
    let ConnectedSftp {
        jump_chain,
        ssh,
        sftp,
    } = connection;
    let _ = sftp.close().await;
    let _ = ssh
        .disconnect(Disconnect::ByApplication, "SFTP session closed", "en")
        .await;
    // Drop the jump chain after the target session is closed so every tunnel
    // stays open until we are fully done with it.
    drop(jump_chain);
}

pub async fn get_or_create_sftp_connection(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
) -> Result<(), String> {
    let key = SftpConnectionKey::from_plan(tab_id, plan)?;

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
