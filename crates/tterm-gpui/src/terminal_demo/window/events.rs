//! Core session events for the prototype window.

use tterm_core::connections::profile_recents;
use tterm_core::events::AppEvent;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::services::AppServices;
use tterm_core::ssh::store::now_unix_ms;

use crate::terminal_demo::binding::TabBinding;
use crate::terminal_demo::binding::scoped::ConnectionState;

pub(super) fn handle_event(binding: &TabBinding, event: AppEvent, services: &AppServices) -> bool {
    let Some(session) = binding.session() else {
        return false;
    };
    let session_nonce = session.session_nonce();
    let target = binding.tab_id();
    match event {
        AppEvent::Ssh(SshEvent::HostKeyPrompt { tab_id, payload }) if tab_id == target => {
            binding.set_host_key_prompt(session_nonce, Some(payload.clone()));
            session.show_host_key(payload);
            true
        }
        AppEvent::Ssh(SshEvent::HostKeyPromptExpired { tab_id, request_id })
            if tab_id == target =>
        {
            let current = binding
                .scoped_state()
                .host_key_prompt()
                .is_some_and(|prompt| prompt.request_id == request_id);
            if !current {
                return false;
            }
            binding.set_host_key_prompt(session_nonce, None);
            session.expire_host_key(&request_id);
            true
        }
        AppEvent::Ssh(SshEvent::AuthPrompt { tab_id, payload }) if tab_id == target => {
            session.show_auth(payload);
            true
        }
        AppEvent::Ssh(SshEvent::AuthPromptExpired { tab_id, request_id }) if tab_id == target => {
            session.expire_auth(&request_id);
            true
        }
        AppEvent::Ssh(SshEvent::ConnectionProgress { tab_id, payload }) if tab_id == target => {
            let state = connection_state(&payload);
            binding.set_connection_progress(session_nonce, Some(payload));
            let changed = binding.set_connection_state(session_nonce, state);
            if changed
                && state == ConnectionState::Connected
                && let Some(profile_id) = binding.saved_profile_id()
            {
                record_profile_use(services, profile_id);
            }
            true
        }
        AppEvent::Terminal(TerminalEvent::Output {
            tab_id,
            session_nonce: output_nonce,
            text,
        }) if tab_id == target => session.accept_output(output_nonce, text.as_bytes()),
        AppEvent::Terminal(TerminalEvent::Exit { tab_id, .. }) if tab_id == target => {
            binding.set_connection_state(session_nonce, ConnectionState::Disconnected);
            true
        }
        _ => false,
    }
}

pub(super) fn connection_state(payload: &SshConnectionProgressPayload) -> ConnectionState {
    match payload.phase.as_str() {
        "ready" => ConnectionState::Connected,
        "retrying" | "serial_waiting" => ConnectionState::Reconnecting,
        "failed" | "retry_exhausted" => ConnectionState::Error,
        _ => ConnectionState::Connecting,
    }
}

/// Credits a saved profile once its own shell is ready, without blocking the
/// window. The event time is captured here so a delayed worker cannot make an
/// older use look newer.
pub(super) fn record_profile_use(services: &AppServices, profile_id: String) {
    let used_at = now_unix_ms();
    drop(services.shared.runtime.spawn_blocking(move || {
        if let Err(error) = profile_recents::record_recent_profile(&profile_id, used_at) {
            eprintln!("tterm-gpui: failed to record a recent saved connection: {error}");
        }
    }));
}
