use russh::client;
use russh::Disconnect;
use russh_sftp::client::error::Error as SftpError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::core::session::{
    normalize_connection, resolve_ssh_password, types::PtyConnectionOptions, types::SessionPlan,
};
use crate::core::state::{HostPromptMap, SessionKind};
use crate::sftp::internal::launcher::ChannelLauncher;
use crate::sftp::internal::sudo::handshake::authenticate;
use crate::sftp::internal::types::{
    CachedSftpConnection, ConnectedSftp, SftpConnectionKey, SftpConnectionPool, SftpHandles,
};
use crate::ssh::auth::{AuthPrompter, AuthTarget};
use crate::ssh::types::ConnectionStatusOptions;
use crate::ssh::{
    jump::connect::open_target_ssh_session, jump::connect::ConnectContext,
    jump::connect::JumpChain, jump::connect::SshTarget, secret_store::types::SecretStoreState,
    types::SshClientHandler,
};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300);
/// How often the pool is checked for idle connections to close.
const REAP_INTERVAL: Duration = Duration::from_secs(60);

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
        ConnectContext {
            app,
            tab_id,
            prompts: &prompts,
            // Progress events share the terminal tab's channel; emitting them
            // would flip an already-connected terminal back to "connecting".
            status_options: ConnectionStatusOptions::QUIET,
            host_key_verification_mode:
                crate::ssh::types::HostKeyVerificationMode::PromptAndPersist,
            auth_prompter: &crate::ssh::auth::AuthPrompter::tab(app, tab_id),
        },
        SshTarget::from_plan(plan, &host, &username),
    )
    .await
    .map_err(String::from)
}

async fn connect_sftp(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
) -> Result<ConnectedSftp, String> {
    let (jump_chain, ssh) = connect_authenticated_ssh(app, tab_id, plan, prompts).await?;

    let launcher = if plan.sftp_sudo {
        let prompter = AuthPrompter::tab(app, tab_id);
        let target = AuthTarget {
            host: plan.host.as_deref().unwrap_or_default(),
            port: plan.port,
            username: plan.username.as_deref().unwrap_or_default(),
            hop: None,
        };
        let auth = authenticate(&ssh, saved_sudo_password(app, plan), |retry| {
            let prompter = prompter.clone();
            async move { prompter.ask_sudo_password(&target, retry).await }
        })
        .await?;
        ChannelLauncher::sudo(auth)
    } else {
        ChannelLauncher::default()
    };
    let sftp = launcher.open_sftp_session(&ssh).await?;

    Ok(ConnectedSftp {
        jump_chain,
        ssh: Arc::new(ssh),
        sftp,
        launcher,
    })
}

/// The saved password a sudo prompt of this connection would be answered
/// with: its sudo password, else its login password. None when there is
/// neither, or the connection turned off filling sudo prompts, which then
/// means asking every time.
async fn saved_sudo_password(
    app: &AppHandle,
    plan: &SessionPlan,
) -> Result<Option<zeroize::Zeroizing<String>>, String> {
    let app = app.clone();
    let profile_id = plan.profile_id.clone();
    let profile_name = plan.profile_name.clone();
    crate::core::blocking::run_blocking(move || {
        let secret_state = app
            .try_state::<SecretStoreState>()
            .ok_or("Saved passwords are unavailable")?;
        let saved = crate::core::session::load_saved_sudo_password(
            &app,
            &secret_state,
            profile_id.as_deref(),
            Some(&profile_name),
        )?;
        Ok(saved.map(|(password, _source)| password))
    })
    .await
}

pub async fn close_sftp(connection: ConnectedSftp) {
    let ConnectedSftp {
        jump_chain,
        ssh,
        sftp,
        launcher: _,
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

/// The tab's pooled connection, opened when there is none: its SFTP session,
/// the SSH connection under it and the launcher for running commands next to
/// it. Holding the handles keeps the reaper from closing the connection
/// mid-operation.
pub async fn pooled_handles(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
) -> Result<SftpHandles, String> {
    let key = SftpConnectionKey::from_plan(tab_id, plan)?;
    get_or_create_sftp_connection(app, tab_id, plan, prompts, pool).await?;
    let pool_guard = pool.read().await;
    let cached = pool_guard.get(&key).ok_or("Connection not found")?;
    Ok(SftpHandles {
        sftp: cached.connection.sftp.clone(),
        ssh: cached.connection.ssh.clone(),
        launcher: cached.connection.launcher.clone(),
    })
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
