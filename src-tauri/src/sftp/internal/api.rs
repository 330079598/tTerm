macro_rules! with_sftp {
    ($app:expr, $tab_id:expr, $plan:expr, $prompts:expr, $pool:expr, $sftp:ident => $body:block) => {{
        let key = $crate::sftp::internal::types::SftpConnectionKey::from_plan($tab_id, $plan)?;

        // Ensure connection exists
        $crate::sftp::internal::connection::get_or_create_sftp_connection(
            $app,
            $tab_id,
            $plan,
            $prompts.clone(),
            $pool,
        )
        .await?;

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
                    $crate::sftp::internal::connection::close_sftp(cached.connection).await;
                });
            }
        }

        result
    }};
}

pub mod base;
pub mod delete;
pub mod download;
pub mod edit;
pub mod upload;

use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{
    evict_connection, get_or_create_sftp_connection, open_sftp_raw_session,
};
use crate::sftp::internal::transfer::RemoteChannels;
use crate::sftp::internal::types::{SftpConnectionKey, SftpConnectionPool};
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tauri::AppHandle;

/// Resolve the configured number of parallel SFTP channels (clamped safely).
pub fn resolve_transfer_parallelism() -> usize {
    crate::config::load_config_file()
        .ok()
        .map(|config| config.sftp_transfer_parallelism as usize)
        .unwrap_or(4)
        .clamp(1, 16)
}

/// Ensure a pooled connection exists, then open `parallelism` SFTP channels
/// over that single SSH connection. Returns the high-level control session and
/// the raw parallel channels.
pub async fn prepare_transfer(
    app: &AppHandle,
    tab_id: &str,
    plan: &SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
) -> Result<(Arc<SftpSession>, RemoteChannels), String> {
    let key = SftpConnectionKey::from_plan(tab_id, plan)?;

    get_or_create_sftp_connection(app, tab_id, plan, prompts, pool).await?;

    let parallelism = resolve_transfer_parallelism();

    // Clone the pooled handles out from under the pool read lock. Channel
    // setup is network I/O (one or more SSH round trips per channel); doing
    // it while holding the lock would block pool writers — and, because the
    // tokio RwLock is write-preferring, every subsequent reader queued behind
    // them — for the whole setup on a slow link.
    let (sftp, ssh) = {
        let pool_guard = pool.read().await;
        let cached = pool_guard.get(&key).ok_or("Connection not found")?;
        (
            cached.connection.sftp.clone(),
            cached.connection.ssh.clone(),
        )
    };

    let mut sessions = Vec::with_capacity(parallelism);
    let mut limits = None;
    let mut open_error = None;
    for _ in 0..parallelism {
        match open_sftp_raw_session(ssh.as_ref()).await {
            Ok((session, negotiated)) => {
                if limits.is_none() {
                    limits = negotiated;
                }
                sessions.push(session);
            }
            Err(error) => {
                open_error = Some(error);
                break;
            }
        }
    }

    if let Some(error) = open_error {
        // A failed channel setup means the pooled connection is unusable.
        evict_connection(pool, &key).await;
        return Err(error);
    }

    Ok((sftp, RemoteChannels::new(sessions, limits)))
}
