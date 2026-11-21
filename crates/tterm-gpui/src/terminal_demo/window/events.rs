//! Core session events for the prototype window.

use tterm_core::events::AppEvent;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;

use crate::terminal_demo::binding::TabBinding;
use crate::terminal_demo::binding::scoped::ConnectionState;

pub(super) fn handle_event(binding: &TabBinding, event: AppEvent) -> bool {
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
            binding.set_connection_state(session_nonce, state);
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
