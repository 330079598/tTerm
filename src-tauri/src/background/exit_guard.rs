//! Asking before a quit that would lose something: running tunnels, or
//! changes to files opened from SFTP that were never uploaded.
//! The backend holds the exit and tells the UI what is at stake; the answer
//! comes back as `confirm_quit_app` or `cancel_quit_prompt`.

use std::sync::Arc;
use tterm_core::events::background::BackgroundEvent;
use tterm_core::events::background::QuitBlockers;
use tterm_core::events::{AppEvent, EventSink};

use std::sync::atomic::{AtomicBool, Ordering};

use crate::tunnel::commands::TunnelManager;

/// Tracks confirmation without holding the window or application lifetime.
#[derive(Default)]
pub struct ExitGuardState {
    /// The user agreed to quit despite what is at stake.
    confirmed: AtomicBool,
    /// A quit confirmation is on screen.
    prompt_open: AtomicBool,
}

impl ExitGuardState {
    fn cancel(&self) {
        self.prompt_open.store(false, Ordering::SeqCst);
    }
    fn should_hold(&self, blockers: &QuitBlockers) -> bool {
        if blockers.is_empty() || self.confirmed.load(Ordering::SeqCst) {
            return false;
        }
        // Asking to quit again while the prompt is open means the user insists
        // (or the prompt could not be shown); never trap them in the app.
        !self.prompt_open.swap(true, Ordering::SeqCst)
    }
}

/// What a quit would end right now.
pub(crate) fn blockers(tunnels: &TunnelManager, unuploaded_files: &[String]) -> QuitBlockers {
    QuitBlockers {
        tunnels: tunnels.active_count(),
        unuploaded_files: unuploaded_files.to_vec(),
    }
}

/// Holds an exit while the UI confirms the current blockers.
pub(crate) fn hold_exit_for_confirmation(
    state: &ExitGuardState,
    blockers: QuitBlockers,
    events: &dyn EventSink,
) -> bool {
    emit_confirmation(state, blockers, events)
}

fn emit_confirmation(
    state: &ExitGuardState,
    blockers: QuitBlockers,
    events: &dyn EventSink,
) -> bool {
    let hold = state.should_hold(&blockers);
    if hold {
        let _ = events.emit(AppEvent::Background(BackgroundEvent::QuitRequested(
            blockers,
        )));
    }
    hold
}

#[tauri::command]
pub fn confirm_quit_app(services: tauri::State<'_, Arc<crate::services::AppServices>>) {
    confirm(
        &services.app.exit_guard,
        &crate::app_context::quit(&services),
    );
}

fn confirm(state: &ExitGuardState, context: &super::quit::QuitContext) {
    state.confirmed.store(true, Ordering::SeqCst);
    super::quit::quit(context);
}

#[tauri::command]
pub fn cancel_quit_prompt(services: tauri::State<'_, Arc<crate::services::AppServices>>) {
    services
        .app
        .exit_guard
        .prompt_open
        .store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_is_held_once_while_something_is_at_stake() {
        let state = ExitGuardState::default();
        assert!(
            !state.should_hold(&QuitBlockers::default()),
            "nothing at stake"
        );

        let tunnels = QuitBlockers {
            tunnels: 2,
            ..QuitBlockers::default()
        };
        assert!(state.should_hold(&tunnels), "first attempt asks the user");
        assert!(!state.should_hold(&tunnels), "insisting lets the app quit");

        state.prompt_open.store(false, Ordering::SeqCst);
        assert!(state.should_hold(&tunnels), "cancelling re-arms the prompt");

        state.confirmed.store(true, Ordering::SeqCst);
        assert!(!state.should_hold(&tunnels), "confirmed");
    }

    #[test]
    fn files_not_uploaded_are_at_stake() {
        let files = QuitBlockers {
            unuploaded_files: vec!["nginx.conf".to_string()],
            ..QuitBlockers::default()
        };
        assert!(!files.is_empty());
        assert!(ExitGuardState::default().should_hold(&files));
    }
}

#[cfg(test)]
#[path = "exit_guard_legacy_tests.rs"]
mod legacy_contract_tests;
