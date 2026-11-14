pub(crate) mod commands;
pub(crate) mod prompts;
mod session;

use crate::monitor::session::{
    collect_metrics_snapshot, get_or_open_monitor_session, monitor_connection_fingerprint,
    reap_idle_monitor_sessions, release_monitor_session_inner,
};
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::monitor::types::MonitorSessionMap;
use tterm_core::monitor::types::ServerMetricsSnapshot;
use tterm_core::ssh::secret_store::types::SecretStoreState;
use tterm_core::{core::session::normalize_connection, core::session::resolve_ssh_password};

struct GetServerMetricsSnapshotInput {
    tab_id: String,
    session_nonce: u32,
    connection: Option<PtyConnectionOptions>,
}

async fn get_server_metrics_snapshot_service(
    app: tterm_core::ssh::context::ConnectionContext,
    prompt_state: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    secret_state: SecretStoreState,
    monitor_sessions: MonitorSessionMap,
    registry: std::sync::Arc<prompts::PendingMonitorPrompts>,
    arguments: GetServerMetricsSnapshotInput,
) -> Result<ServerMetricsSnapshot, String> {
    let GetServerMetricsSnapshotInput {
        tab_id,
        session_nonce,
        connection,
    } = arguments;

    let mut plan = normalize_connection(connection)?;
    if !matches!(plan.kind, tterm_core::core::state::SessionKind::Ssh) {
        return Err("Server monitoring requires an SSH connection".to_string());
    }
    resolve_ssh_password(&secret_state, &mut plan)?;
    reap_idle_monitor_sessions(monitor_sessions.clone()).await;

    let fingerprint = monitor_connection_fingerprint(&plan)?;
    let monitor_session = get_or_open_monitor_session(
        session::MonitorContext {
            connection: &app,
            registry: &registry,
        },
        &tab_id,
        session_nonce,
        &plan,
        &fingerprint,
        prompt_state.clone(),
        monitor_sessions.clone(),
    )
    .await?;

    match collect_metrics_snapshot(monitor_session).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            release_monitor_session_inner(
                monitor_sessions.clone(),
                &tab_id,
                Some(session_nonce),
                "monitor metrics failed",
            )
            .await;
            Err(error)
        }
    }
}

struct ReleaseServerMonitorSessionInput {
    tab_id: String,
    session_nonce: Option<u32>,
}

async fn release_server_monitor_session_service(
    app: tterm_core::ssh::context::ConnectionContext,
    monitor_sessions: MonitorSessionMap,
    registry: std::sync::Arc<prompts::PendingMonitorPrompts>,
    arguments: ReleaseServerMonitorSessionInput,
) -> Result<(), String> {
    let ReleaseServerMonitorSessionInput {
        tab_id,
        session_nonce,
    } = arguments;

    registry.cancel(&app.prompts, &tab_id, session_nonce);
    reap_idle_monitor_sessions(monitor_sessions.clone()).await;
    release_monitor_session_inner(
        monitor_sessions.clone(),
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
    use tterm_core::core::session::types::{JumpHostPlan, SessionPlan};
    use tterm_core::core::state::SessionKind;

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
            reconnect_enabled: false,
            reconnect_max_attempts: 0,
            private_key_path: None,
            private_key_passphrase: None,
            certificate_path: None,
            use_agent: false,
            keyboard_interactive: false,
            auto_auth: false,
            agent_forward: false,
            terminal_shell: None,
            jump_hosts: Vec::new(),
            encoding: Default::default(),
            shell_integration: false,
            session_log: Default::default(),
            proxy_choice: Default::default(),
            proxy: None,
            serial: None,
            sftp_sudo: false,
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
            certificate_path: None,
            use_agent: false,
            keyboard_interactive: false,
            auto_auth: false,
            profile_id: None,
        });

        let mut second = ssh_plan("target", "user", "profile");
        second.jump_hosts.push(JumpHostPlan {
            host: "a".to_string(),
            port: 22,
            username: "b|jump".to_string(),
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
            certificate_path: None,
            use_agent: false,
            keyboard_interactive: false,
            auto_auth: false,
            profile_id: None,
        });

        assert_ne!(
            monitor_connection_fingerprint(&first).unwrap(),
            monitor_connection_fingerprint(&second).unwrap()
        );
    }
}
