pub(crate) mod metrics;
pub(crate) mod prompts;
mod session;
pub(crate) mod types;

use crate::core::session::{
    normalize_connection, resolve_ssh_password, types::PtyConnectionOptions,
};
use crate::ssh::secret_store::types::SecretStoreState;
use session::{
    collect_metrics_snapshot, get_or_open_monitor_session, monitor_connection_fingerprint,
    reap_idle_monitor_sessions, release_monitor_session_inner,
};
use types::MonitorSessionMap;
use types::ServerMetricsSnapshot;

#[tauri::command]
pub async fn get_server_metrics_snapshot(
    tab_id: String,
    session_nonce: u32,
    connection: Option<PtyConnectionOptions>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ServerMetricsSnapshot, String> {
    let app = crate::connection_context::connection(&services);
    let prompt_state = services.shared.prompts.clone();
    let secret_state = services.shared.secrets.clone();
    let monitor_sessions = services.session.monitors.clone();
    let registry = services.session.monitor_prompts.clone();
    get_server_metrics_snapshot_service(
        app,
        prompt_state,
        secret_state,
        monitor_sessions,
        registry,
        (tab_id, session_nonce, connection),
    )
    .await
}
async fn get_server_metrics_snapshot_service(
    app: crate::ssh::context::ConnectionContext,
    prompt_state: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    secret_state: SecretStoreState,
    monitor_sessions: MonitorSessionMap,
    registry: std::sync::Arc<prompts::PendingMonitorPrompts>,
    arguments: (String, u32, Option<PtyConnectionOptions>),
) -> Result<ServerMetricsSnapshot, String> {
    let (tab_id, session_nonce, connection) = arguments;

    let mut plan = normalize_connection(connection)?;
    if !matches!(plan.kind, crate::core::state::SessionKind::Ssh) {
        return Err("Server monitoring requires an SSH connection".to_string());
    }
    resolve_ssh_password(&secret_state, &mut plan)?;
    reap_idle_monitor_sessions(monitor_sessions.clone()).await;

    let fingerprint = monitor_connection_fingerprint(&plan)?;
    let monitor_session = get_or_open_monitor_session(
        (&app, &registry),
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

#[tauri::command]
pub async fn release_server_monitor_session(
    tab_id: String,
    session_nonce: Option<u32>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::connection(&services);
    let monitor_sessions = services.session.monitors.clone();
    let registry = services.session.monitor_prompts.clone();
    release_server_monitor_session_service(app, monitor_sessions, registry, (tab_id, session_nonce))
        .await
}
async fn release_server_monitor_session_service(
    app: crate::ssh::context::ConnectionContext,
    monitor_sessions: MonitorSessionMap,
    registry: std::sync::Arc<prompts::PendingMonitorPrompts>,
    arguments: (String, Option<u32>),
) -> Result<(), String> {
    let (tab_id, session_nonce) = arguments;

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
    use crate::core::session::{types::JumpHostPlan, types::SessionPlan};
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
