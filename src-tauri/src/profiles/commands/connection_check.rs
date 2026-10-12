//! The SSH session behind the profile dialog's connection test.

use crate::core::state::HostPromptMap;
use crate::ssh::{ConnectionStatusOptions, HostKeyVerificationMode, SshClientHandler};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub(super) fn test_connection_handler(
    app: &tauri::AppHandle,
    tab_id: &str,
    plan: &crate::core::session::SessionPlan,
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
    plan: &crate::core::session::SessionPlan,
    auth_prompter: &crate::ssh::AuthPrompter,
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
