use russh::client;
use russh::Disconnect;
use russh_sftp::client::rawsession::Limits;
use russh_sftp::client::{error::Error as SftpError, RawSftpSession, SftpSession};
use russh_sftp::extensions::LimitsExtension;
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
use crate::ssh::ConnectionStatusOptions;
use crate::ssh::{open_target_ssh_session, JumpChain, SecretStoreState, SshClientHandler};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300);
/// How often the pool is checked for idle connections to close.
const REAP_INTERVAL: Duration = Duration::from_secs(60);
const SFTP_REQUEST_TIMEOUT_SECS: u64 = 120;
/// Bound on opening an SSH channel and negotiating the SFTP subsystem on it.
/// `russh_sftp` only times out its own protocol requests (via
/// `set_timeout`), which only start once a channel exists — the
/// `channel_open_session`/`request_subsystem` calls that precede it are
/// plain `russh` futures with no timeout of their own. A server that accepts
/// a channel but never answers it (e.g. a session limit that is enforced by
/// silence rather than a refusal, or a transport that stalls without
/// resetting the socket) would otherwise hang this forever — freezing a
/// fresh transfer at 0% with no error, since `prepare_transfer` opens a new
/// batch of channels for every upload/download call and waits for all of
/// them before doing any I/O.
const SFTP_CHANNEL_SETUP_TIMEOUT: Duration = Duration::from_secs(30);

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
        plan.auth_method(),
        plan.agent_forward,
        plan.keepalive_interval_secs,
        plan.keepalive_count_max,
        &plan.jump_hosts,
        prompts,
        // Progress events share the terminal tab's channel; emitting them
        // would flip an already-connected terminal back to "connecting".
        ConnectionStatusOptions::QUIET,
        crate::ssh::HostKeyVerificationMode::PromptAndPersist,
        &crate::ssh::AuthPrompter::tab(app, tab_id),
    )
    .await
    .map_err(String::from)
}

async fn open_control_sftp_session(
    ssh: &client::Handle<SshClientHandler>,
) -> Result<Arc<SftpSession>, String> {
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
    sftp.set_timeout(SFTP_REQUEST_TIMEOUT_SECS);
    Ok(Arc::new(sftp))
}

async fn connect_sftp(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<ConnectedSftp, String> {
    let (jump_chain, ssh) = connect_authenticated_ssh(app, tab_id, plan, prompts).await?;

    let sftp = tokio::time::timeout(SFTP_CHANNEL_SETUP_TIMEOUT, open_control_sftp_session(&ssh))
        .await
        .map_err(|_| "Timed out opening SFTP channel".to_string())??;

    Ok(ConnectedSftp {
        jump_chain,
        ssh: Arc::new(ssh),
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
        cached.released = false;
        return Ok(());
    }
    drop(pool_guard);

    let connection = connect_sftp(app, tab_id, plan, prompts).await?;

    let mut pool_guard = pool.write().await;
    pool_guard.insert(
        key,
        CachedSftpConnection {
            connection,
            last_used: now,
            released: false,
        },
    );

    Ok(())
}

/// An operation holds a clone of the session handles while it runs
/// (`with_sftp!`, transfers); closing the connection then would cut it off.
fn in_use(connection: &ConnectedSftp) -> bool {
    Arc::strong_count(&connection.sftp) > 1 || Arc::strong_count(&connection.ssh) > 1
}

fn is_reapable(released: bool, idle: Duration, in_use: bool) -> bool {
    !in_use && (released || idle > CONNECTION_TIMEOUT)
}

/// Closes the pooled connections no operation is using that have sat idle
/// past `CONNECTION_TIMEOUT` or whose tab has closed.
pub async fn reap_idle_connections(pool: &SftpConnectionPool) {
    let now = Instant::now();
    let mut pool_guard = pool.write().await;
    let reapable: Vec<_> = pool_guard
        .iter()
        .filter(|(_, cached)| {
            is_reapable(
                cached.released,
                now.duration_since(cached.last_used),
                in_use(&cached.connection),
            )
        })
        .map(|(key, _)| key.clone())
        .collect();

    for key in reapable {
        if let Some(cached) = pool_guard.remove(&key) {
            tokio::spawn(async move {
                close_sftp(cached.connection).await;
            });
        }
    }
}

/// Closes a closed tab's pooled connections: right away when idle, otherwise
/// on the first reap after the running operation finishes.
pub async fn release_tab_connections(pool: &SftpConnectionPool, tab_id: &str) {
    {
        let mut pool_guard = pool.write().await;
        for (key, cached) in pool_guard.iter_mut() {
            if key.tab_id == tab_id {
                cached.released = true;
            }
        }
    }
    reap_idle_connections(pool).await;
}

/// Reaps the pool every `REAP_INTERVAL` for the life of the app, so idle
/// connections close even when no new SFTP connection is opened.
pub fn spawn_idle_reaper(pool: SftpConnectionPool) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(REAP_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            reap_idle_connections(&pool).await;
        }
    });
}

/// Remove a pooled connection (used when it is known to be broken) and close
/// it in the background.
pub async fn evict_connection(pool: &SftpConnectionPool, key: &SftpConnectionKey) {
    let mut pool_guard = pool.write().await;
    if let Some(cached) = pool_guard.remove(key) {
        tokio::spawn(async move {
            close_sftp(cached.connection).await;
        });
    }
}

/// Open one additional SFTP subsystem channel on an existing SSH connection and
/// wrap it as a raw session, negotiating `limits@openssh.com` when offered.
pub async fn open_sftp_raw_session(
    ssh: &client::Handle<SshClientHandler>,
) -> Result<(Arc<RawSftpSession>, Option<LimitsExtension>), String> {
    tokio::time::timeout(SFTP_CHANNEL_SETUP_TIMEOUT, open_sftp_raw_session_inner(ssh))
        .await
        .map_err(|_| "Timed out opening SFTP channel".to_string())?
}

async fn open_sftp_raw_session_inner(
    ssh: &client::Handle<SshClientHandler>,
) -> Result<(Arc<RawSftpSession>, Option<LimitsExtension>), String> {
    let channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SFTP channel: {err}"))?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|err| format!("Failed to start SFTP subsystem: {err}"))?;

    let mut session = RawSftpSession::new(channel.into_stream());
    let version = session
        .init()
        .await
        .map_err(|err| format!("Failed to initialize SFTP channel: {err}"))?;

    session.set_timeout(SFTP_REQUEST_TIMEOUT_SECS);

    let limits = if version
        .extensions
        .get(russh_sftp::extensions::LIMITS)
        .is_some_and(|value| value == "1")
    {
        let extension = session
            .limits()
            .await
            .map_err(|err| format!("Failed to query server SFTP limits: {err}"))?;
        session.set_limits(Limits {
            packet_len: None,
            read_len: (extension.max_read_len > 0).then_some(extension.max_read_len),
            write_len: (extension.max_write_len > 0).then_some(extension.max_write_len),
            open_handles: None,
        });
        Some(extension)
    } else {
        None
    };

    Ok((Arc::new(session), limits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn released_connections_close_once_unused() {
        assert!(is_reapable(true, Duration::ZERO, false));
        assert!(!is_reapable(true, Duration::ZERO, true));
    }

    #[test]
    fn idle_connections_close_after_the_timeout_unless_in_use() {
        let past_timeout = CONNECTION_TIMEOUT + Duration::from_secs(1);
        assert!(!is_reapable(false, CONNECTION_TIMEOUT, false));
        assert!(is_reapable(false, past_timeout, false));
        assert!(!is_reapable(false, past_timeout, true));
    }
}
