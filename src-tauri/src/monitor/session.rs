use super::metrics::parse_metrics_output;
use super::types::{MonitorSession, MonitorSessionMap, MONITOR_SESSION_IDLE_TIMEOUT};
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::{ConnectionStatusOptions, open_target_ssh_session};
use russh::ChannelMsg;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Instant;

pub(crate) fn monitor_connection_fingerprint(plan: &SessionPlan) -> Result<String, String> {
    let host = plan
        .host
        .as_deref()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .as_deref()
        .ok_or_else(|| "SSH username is required".to_string())?;

    let jump_chain = plan
        .jump_hosts
        .iter()
        .map(|jump| {
            (
                jump.host.as_str(),
                jump.port,
                jump.username.as_str(),
                jump.private_key_path.as_deref().unwrap_or(""),
                jump.private_key_passphrase.is_some(),
            )
        })
        .collect::<Vec<_>>();

    serde_json::to_string(&(
        plan.profile_id.as_deref().unwrap_or(""),
        plan.profile_name.as_str(),
        host,
        plan.port,
        username,
        plan.private_key_path.as_deref().unwrap_or(""),
        plan.private_key_passphrase.is_some(),
        plan.keepalive_interval_secs,
        jump_chain,
    ))
    .map_err(|err| format!("Failed to fingerprint monitor connection: {err}"))
}

pub(crate) async fn get_or_open_monitor_session(
    app: &tauri::AppHandle,
    tab_id: &str,
    session_nonce: u32,
    plan: &SessionPlan,
    fingerprint: &str,
    prompts: HostPromptMap,
    monitor_sessions: MonitorSessionMap,
) -> Result<Arc<Mutex<MonitorSession>>, String> {
    let existing_session = {
        let sessions = monitor_sessions.lock().await;
        sessions.get(tab_id).cloned()
    };

    if let Some(session) = existing_session {
        let matches_request = {
            let mut session_guard = session.lock().await;
            session_guard.last_used_at = Instant::now();
            session_guard.session_nonce == session_nonce && session_guard.fingerprint == fingerprint
        };
        if matches_request {
            return Ok(session);
        }
    }

    let stale_session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.remove(tab_id)
    };
    close_monitor_session(stale_session, "monitor session replaced").await;

    let host = plan
        .host
        .clone()
        .ok_or_else(|| "SSH host is required".to_string())?;
    let username = plan
        .username
        .clone()
        .ok_or_else(|| "SSH username is required".to_string())?;

    let (jump_chain, ssh) = open_target_ssh_session(
        &app,
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
        ConnectionStatusOptions::QUIET,
        crate::ssh::HostKeyVerificationMode::PromptAndPersist,
    )
    .await?;

    let cached_session = Arc::new(Mutex::new(MonitorSession {
        session_nonce,
        fingerprint: fingerprint.to_string(),
        last_used_at: Instant::now(),
        jump_chain,
        ssh,
    }));

    let replaced_session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.insert(tab_id.to_string(), cached_session.clone())
    };
    close_monitor_session(replaced_session, "monitor session replaced").await;
    schedule_monitor_session_idle_timeout(
        monitor_sessions.clone(),
        tab_id.to_string(),
        cached_session.clone(),
    );

    Ok(cached_session)
}

pub(crate) async fn collect_metrics_snapshot(
    monitor_session: Arc<Mutex<MonitorSession>>,
) -> Result<super::types::ServerMetricsSnapshot, String> {
    let mut session = monitor_session.lock().await;
    session.last_used_at = Instant::now();
    let mut channel = session
        .ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    channel
        .exec(true, super::metrics::METRICS_SCRIPT)
        .await
        .map_err(|err| format!("Failed to execute metrics command: {err}"))?;

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_status = None;
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => stdout.push_str(&String::from_utf8_lossy(&data)),
            ChannelMsg::ExtendedData { data, .. } => {
                stderr.push_str(&String::from_utf8_lossy(&data))
            }
            ChannelMsg::ExitStatus { exit_status: code } => exit_status = Some(code),
            ChannelMsg::Close => break,
            _ => {}
        }
    }

    let _ = channel.close().await;
    session.last_used_at = Instant::now();

    if exit_status.unwrap_or(0) != 0 {
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!(
                "Metrics command failed with exit status {}",
                exit_status.unwrap_or(1)
            )
        } else {
            format!(
                "Metrics command failed with exit status {}: {}",
                exit_status.unwrap_or(1),
                detail
            )
        });
    }

    Ok(parse_metrics_output(&stdout))
}

pub(crate) async fn reap_idle_monitor_sessions(monitor_sessions: MonitorSessionMap) {
    let sessions = {
        let sessions = monitor_sessions.lock().await;
        sessions
            .iter()
            .map(|(tab_id, session)| (tab_id.clone(), session.clone()))
            .collect::<Vec<_>>()
    };
    let now = Instant::now();
    let mut idle_sessions = Vec::new();

    for (tab_id, session) in sessions {
        let is_idle = {
            let session = session.lock().await;
            now.duration_since(session.last_used_at) >= MONITOR_SESSION_IDLE_TIMEOUT
        };
        if is_idle {
            idle_sessions.push((tab_id, session));
        }
    }

    for (tab_id, idle_session) in idle_sessions {
        let session_to_close = {
            let mut sessions = monitor_sessions.lock().await;
            let should_remove = sessions
                .get(&tab_id)
                .is_some_and(|current| Arc::ptr_eq(current, &idle_session));
            should_remove.then(|| sessions.remove(&tab_id)).flatten()
        };
        close_monitor_session(session_to_close, "monitor session idle timeout").await;
    }
}

fn schedule_monitor_session_idle_timeout(
    monitor_sessions: MonitorSessionMap,
    tab_id: String,
    watched_session: Arc<Mutex<MonitorSession>>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(MONITOR_SESSION_IDLE_TIMEOUT).await;

            let is_still_current = {
                let sessions = monitor_sessions.lock().await;
                sessions
                    .get(&tab_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &watched_session))
            };
            if !is_still_current {
                break;
            }

            let is_idle = {
                let session = watched_session.lock().await;
                Instant::now().duration_since(session.last_used_at) >= MONITOR_SESSION_IDLE_TIMEOUT
            };
            if !is_idle {
                continue;
            }

            let session_to_close = {
                let mut sessions = monitor_sessions.lock().await;
                let should_remove = sessions
                    .get(&tab_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &watched_session));
                should_remove.then(|| sessions.remove(&tab_id)).flatten()
            };
            close_monitor_session(session_to_close, "monitor session idle timeout").await;
            break;
        }
    });
}

pub(crate) async fn release_monitor_session_inner(
    monitor_sessions: MonitorSessionMap,
    tab_id: &str,
    session_nonce: Option<u32>,
    reason: &'static str,
) {
    let session = {
        let mut sessions = monitor_sessions.lock().await;
        sessions.remove(tab_id)
    };

    if let Some(session) = session {
        if let Some(nonce) = session_nonce {
            let matches_nonce = {
                let session_guard = session.lock().await;
                session_guard.session_nonce == nonce
            };
            if !matches_nonce {
                let mut session_to_close = None;
                let mut sessions = monitor_sessions.lock().await;
                if sessions.contains_key(tab_id) {
                    session_to_close = Some(session);
                } else {
                    sessions.insert(tab_id.to_string(), session);
                }
                drop(sessions);
                close_monitor_session(session_to_close, "monitor session superseded").await;
                return;
            }
        }

        close_monitor_session(Some(session), reason).await;
    }
}

async fn close_monitor_session(session: Option<Arc<Mutex<MonitorSession>>>, reason: &'static str) {
    if let Some(session) = session {
        let mut session = session.lock().await;
        if let Err(err) = session
            .ssh
            .disconnect(russh::Disconnect::ByApplication, reason, "en")
            .await
        {
            eprintln!("Failed to disconnect monitor session ({reason}): {err}");
        }
        drop(session.jump_chain.take());
    }
}
