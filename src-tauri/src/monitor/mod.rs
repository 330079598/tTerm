mod metrics;
mod session;
mod types;

use crate::core::session::{normalize_connection, resolve_ssh_password, PtyConnectionOptions};
use crate::core::state::HostPromptMap;
use crate::ssh::SecretStoreState;
pub use types::MonitorSessionMap;
use session::{
    collect_metrics_snapshot, get_or_open_monitor_session, monitor_connection_fingerprint,
    reap_idle_monitor_sessions, release_monitor_session_inner,
};
use tauri::{AppHandle, State};
use types::ServerMetricsSnapshot;

#[tauri::command]
pub async fn get_server_metrics_snapshot(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    connection: Option<PtyConnectionOptions>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    monitor_sessions: State<'_, MonitorSessionMap>,
) -> Result<ServerMetricsSnapshot, String> {
    let mut plan = normalize_connection(connection)?;
    if !matches!(plan.kind, crate::core::state::SessionKind::Ssh) {
        return Err("Server monitoring requires an SSH connection".to_string());
    }
    resolve_ssh_password(&app, &secret_state, &mut plan)?;
    reap_idle_monitor_sessions(monitor_sessions.inner().clone()).await;

    let fingerprint = monitor_connection_fingerprint(&plan)?;
    let monitor_session = get_or_open_monitor_session(
        &app,
        &tab_id,
        session_nonce,
        &plan,
        &fingerprint,
        prompt_state.inner().clone(),
        monitor_sessions.inner().clone(),
    )
    .await?;

    match collect_metrics_snapshot(monitor_session).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            release_monitor_session_inner(
                monitor_sessions.inner().clone(),
                &tab_id,
                Some(session_nonce),
                "monitor metrics failed",
            )
            .await;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn release_server_monitor_session(
    tab_id: String,
    session_nonce: Option<u32>,
    monitor_sessions: State<'_, MonitorSessionMap>,
) -> Result<(), String> {
    reap_idle_monitor_sessions(monitor_sessions.inner().clone()).await;
    release_monitor_session_inner(
        monitor_sessions.inner().clone(),
        &tab_id,
        session_nonce,
        "monitor session released",
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::session::{JumpHostPlan, SessionPlan};
    use crate::core::state::SessionKind;

    fn ssh_plan(host: &str, username: &str, profile_name: &str) -> SessionPlan {
        SessionPlan {
            kind: SessionKind::Ssh,
            profile_id: None,
            profile_name: profile_name.to_string(),
            host: Some(host.to_string()),
            port: 22,
            username: Some(username.to_string()),
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            keepalive_interval_secs: 15,
            keepalive_count_max: 3,
            private_key_path: None,
            private_key_passphrase: None,
            terminal_shell: None,
            jump_hosts: Vec::new(),
        }
    }

    #[test]
    fn monitor_connection_fingerprint_distinguishes_embedded_separators() {
        let first = ssh_plan("b", "c", "a|");
        let second = ssh_plan("|b", "c", "a");

        assert_ne!(
            monitor_connection_fingerprint(&first).unwrap(),
            monitor_connection_fingerprint(&second).unwrap()
        );
    }

    #[test]
    fn monitor_connection_fingerprint_distinguishes_jump_host_boundaries() {
        let mut first = ssh_plan("target", "user", "profile");
        first.jump_hosts.push(JumpHostPlan {
            host: "a|b".to_string(),
            port: 22,
            username: "jump".to_string(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
        });

        let mut second = ssh_plan("target", "user", "profile");
        second.jump_hosts.push(JumpHostPlan {
            host: "a".to_string(),
            port: 22,
            username: "b|jump".to_string(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
        });

        assert_ne!(
            monitor_connection_fingerprint(&first).unwrap(),
            monitor_connection_fingerprint(&second).unwrap()
        );
    }
}
