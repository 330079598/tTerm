//! Asking before a quit that would lose something: running tunnels, or
//! changes to files opened from SFTP that were never uploaded.
//! The backend holds the exit and tells the UI what is at stake; the answer
//! comes back as `confirm_quit_app` or `cancel_quit_prompt`.

use std::sync::Arc;
use tterm_core::events::background::BackgroundEvent;
use tterm_core::events::background::QuitBlockers;
use tterm_core::events::{AppEvent, EventSink};

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, State};

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
pub fn blockers(app: &AppHandle) -> QuitBlockers {
    QuitBlockers {
        tunnels: app.state::<Arc<TunnelManager>>().active_count(),
        unuploaded_files: crate::sftp::internal::api::external::unuploaded_files(app),
    }
}

/// Called when the app is about to close. Returns `true` when the exit must
/// wait because the UI is asking the user to confirm it.
pub fn hold_exit_for_confirmation(app: &AppHandle) -> bool {
    let blockers = blockers(app);
    emit_confirmation(
        &app.state::<Arc<ExitGuardState>>(),
        blockers,
        &crate::event_sink::TauriEventSink::new(app.clone()),
    )
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
pub fn confirm_quit_app(app: AppHandle, state: State<'_, Arc<ExitGuardState>>) {
    state.confirmed.store(true, Ordering::SeqCst);
    super::quit::quit(&app);
}

#[tauri::command]
pub fn cancel_quit_prompt(state: State<'_, Arc<ExitGuardState>>) {
    state.prompt_open.store(false, Ordering::SeqCst);
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
