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
use crate::sftp::internal::transfer::{RemoteChannels, DEFAULT_PARALLELISM, MAX_PARALLELISM};
use crate::sftp::internal::types::{SftpConnectionKey, SftpConnectionPool};
use russh_sftp::client::SftpSession;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tauri::AppHandle;

/// Resolved parallelism cached against the config file's mtime: transfers
/// resolve this per file (directory batches), and re-reading + re-parsing the
/// config from disk each time shows up as per-file startup latency.
static PARALLELISM_CACHE: RwLock<Option<(Option<SystemTime>, usize)>> = RwLock::new(None);

/// Cache lookup for [`resolve_transfer_parallelism`]: a hit requires the
/// whole mtime token — including `None` ("config file absent") — to match,
/// so a machine running on default config still hits the cache instead of
/// stat-ing the disk on every transfer.
fn parallelism_cache_hit(
    cached: Option<(Option<SystemTime>, usize)>,
    mtime: Option<SystemTime>,
) -> Option<usize> {
    cached
        .filter(|(cached_mtime, _)| *cached_mtime == mtime)
        .map(|(_, value)| value)
}

/// Resolve the configured number of parallel SFTP channels (clamped safely).
pub fn resolve_transfer_parallelism() -> usize {
    let mtime = crate::config::get_config_path()
        .ok()
        .map(|dir| dir.join("config.json"))
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());
    if let Some(cached) = parallelism_cache_hit(
        *PARALLELISM_CACHE.read().expect("parallelism cache read"),
        mtime,
    ) {
        return cached;
    }

    let parallelism = crate::config::load_config_file()
        .ok()
        .map(|config| config.sftp_transfer_parallelism as usize)
        .unwrap_or(DEFAULT_PARALLELISM)
        .clamp(1, MAX_PARALLELISM);

    if let Ok(mut cache) = PARALLELISM_CACHE.write() {
        *cache = Some((mtime, parallelism));
    }
    parallelism
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

    // Open every channel concurrently: sequentially they cost one (or more)
    // round trips each, which is the bulk of the "first transfer feels slow"
    // wait on high-latency links.
    let mut open_tasks = tokio::task::JoinSet::new();
    for _ in 0..parallelism {
        let ssh = ssh.clone();
        open_tasks.spawn(async move { open_sftp_raw_session(ssh.as_ref()).await });
    }

    let mut sessions = Vec::with_capacity(parallelism);
    let mut limits = None;
    let mut open_error = None;
    // Drain every spawned open even when one fails: the requests are already
    // in flight, so aborting them would leak half-opened channels on the
    // connection — and every channel that did open is still usable.
    while let Some(result) = open_tasks.join_next().await {
        match result {
            Ok(Ok((session, negotiated))) => {
                if limits.is_none() {
                    limits = negotiated;
                }
                sessions.push(session);
            }
            Ok(Err(error)) => {
                if open_error.is_none() {
                    open_error = Some(error);
                }
            }
            Err(join_error) => {
                if open_error.is_none() {
                    open_error = Some(format!("SFTP channel open failed: {join_error}"));
                }
            }
        }
    }

    if let Some(error) = open_error {
        if sessions.is_empty() {
            // Every channel failed. Only evict the pooled connection when
            // the transport itself is gone: a live connection that merely
            // refuses new channels (e.g. a tight MaxSessions limit) must
            // stay — the terminal and other SFTP work on this tab run over
            // it, and evicting would tear them all down.
            if ssh.is_closed() {
                evict_connection(pool, &key).await;
            }
            return Err(error);
        }
    }

    // Elastic degradation: with at least one channel open, run the transfer
    // with as many lanes as we actually got instead of failing the transfer
    // over a rejected extra channel.
    Ok((sftp, RemoteChannels::new(sessions, limits)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallelism_cache_hits_when_config_file_is_absent() {
        assert_eq!(parallelism_cache_hit(None, None), None);
        assert_eq!(
            parallelism_cache_hit(Some((None, 4)), None),
            Some(4),
            "a missing config.json must still hit the cache"
        );
        // A config file created after the cache was populated invalidates it.
        assert_eq!(
            parallelism_cache_hit(Some((None, 4)), Some(SystemTime::UNIX_EPOCH)),
            None
        );
    }

    #[test]
    fn parallelism_cache_matches_mtime_token() {
        let mtime = Some(SystemTime::UNIX_EPOCH);
        assert_eq!(parallelism_cache_hit(Some((mtime, 6)), mtime), Some(6));
        assert_eq!(
            parallelism_cache_hit(Some((Some(SystemTime::now()), 6)), mtime),
            None
        );
    }
}
