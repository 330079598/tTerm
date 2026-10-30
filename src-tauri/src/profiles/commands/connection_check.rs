//! The SSH session behind the profile dialog's connection test.

use super::super::storage::normalize_profile;
use super::super::types::SavedProfile;
use crate::core::session::types::SessionPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::{
    types::ConnectionStatusOptions, types::HostKeyVerificationMode, types::SshClientHandler,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub(super) fn test_connection_handler(
    app: &tauri::AppHandle,
    tab_id: &str,
    plan: &crate::core::session::types::SessionPlan,
    host: &str,
    port: u16,
    prompts: HostPromptMap,
    host_key_verification_mode: HostKeyVerificationMode,
) -> SshClientHandler {
    SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_id: plan.profile_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.to_string(),
        port,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        status_options: ConnectionStatusOptions::SILENT,
        host_key_verification_mode,
        forwarded_tcpip_tx: None,
        // A test connection just probes auth; it never keeps the session
        // around long enough for agent forwarding to matter.
        agent_forward_enabled: false,
    }
}

pub(super) async fn authenticate_test_connection<H: russh::client::Handler>(
    session: &mut russh::client::Handle<H>,
    host: &str,
    port: u16,
    username: &str,
    plan: &crate::core::session::types::SessionPlan,
    auth_prompter: &crate::ssh::auth::AuthPrompter,
) -> Result<(), String> {
    crate::ssh::auth::authenticate(
        session,
        &crate::ssh::auth::AuthTarget {
            host,
            port,
            username,
            hop: None,
        },
        plan.auth_method(),
        auth_prompter,
    )
    .await
    .map(|_| ())
    .map_err(String::from)
}

/// The plan a connection test of `profile` runs, before its saved passwords
/// are filled in.
pub(super) fn test_session_plan(
    mut profile: SavedProfile,
    host: &str,
    username: &str,
    port: u16,
) -> SessionPlan {
    normalize_profile(&mut profile);

    let jump_hosts = if profile.uses_jump_host() {
        profile
            .jump_hosts
            .into_iter()
            .map(|j| crate::core::session::types::JumpHostPlan {
                host: j.host,
                port: j.port,
                username: j.username,
                password: if matches!(j.auth_method.as_str(), "agent" | "interactive") {
                    None
                } else {
                    j.password
                },
                private_key_path: if j.auth_method == "key" {
                    j.private_key_path
                } else {
                    None
                },
                private_key_passphrase: if j.auth_method == "key" {
                    j.private_key_passphrase
                } else {
                    None
                },
                certificate_path: if j.auth_method == "key" {
                    j.certificate_path
                } else {
                    None
                },
                use_agent: j.auth_method == "agent",
                auto_auth: j.auth_method == "auto",
                keyboard_interactive: j.auth_method == "interactive",
                profile_id: j.profile_id,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    SessionPlan {
        kind: crate::core::state::SessionKind::Ssh,
        profile_id: Some(profile.id.clone()),
        profile_name: profile.name.clone(),
        host: Some(host.to_string()),
        port,
        username: Some(username.to_string()),
        password: if matches!(
            profile.auth_method.as_deref(),
            Some("agent" | "interactive")
        ) {
            None
        } else {
            profile.password.clone()
        },
        ignore_saved_password: profile.ignore_saved_password,
        remember_password: false,
        private_key_path: if profile.auth_method.as_deref() == Some("key") {
            profile.private_key_path.clone()
        } else {
            None
        },
        private_key_passphrase: profile.private_key_passphrase.clone(),
        certificate_path: if profile.auth_method.as_deref() == Some("key") {
            profile.certificate_path.clone()
        } else {
            None
        },
        use_agent: profile.auth_method.as_deref() == Some("agent"),
        auto_auth: profile.auth_method.as_deref() == Some("auto"),
        keyboard_interactive: profile.auth_method.as_deref() == Some("interactive"),
        agent_forward: false,
        terminal_shell: None,
        keepalive_interval_secs: profile.keepalive_interval_secs as u16,
        keepalive_count_max: profile.keepalive_count_max as u16,
        reconnect_enabled: false,
        reconnect_max_attempts: 0,
        jump_hosts,
        encoding: Default::default(),
        shell_integration: false,
        session_log: Default::default(),
        proxy_choice: profile.proxy.clone(),
        proxy: None,
        serial: None,
    }
}
