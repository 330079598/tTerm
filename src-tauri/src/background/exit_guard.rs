//! Asking before a quit that would lose something, such as running tunnels.
//! The backend holds the exit and tells the UI what is at stake; the answer
//! comes back as `confirm_quit_app` or `cancel_quit_prompt`.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::tunnel::commands::TunnelManager;

/// Emitted when the app is asked to quit with something at stake; carries
/// the [`QuitBlockers`].
const QUIT_REQUESTED_EVENT: &str = "quit-requested";

/// What a quit would end.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuitBlockers {
    /// Port-forwarding tunnels running.
    pub tunnels: usize,
}

impl QuitBlockers {
    pub fn is_empty(&self) -> bool {
        self.tunnels == 0
    }
}

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
        tunnels: app.state::<TunnelManager>().active_count(),
    }
}

/// Called when the app is about to close. Returns `true` when the exit must
/// wait because the UI is asking the user to confirm it.
pub fn hold_exit_for_confirmation(app: &AppHandle) -> bool {
    let blockers = blockers(app);
    let hold = app.state::<ExitGuardState>().should_hold(&blockers);
    if hold {
        let _ = app.emit(QUIT_REQUESTED_EVENT, &blockers);
    }
    hold
}

#[tauri::command]
pub fn confirm_quit_app(app: AppHandle, state: State<'_, ExitGuardState>) {
    state.confirmed.store(true, Ordering::SeqCst);
    super::quit::quit(&app);
}

#[tauri::command]
pub fn cancel_quit_prompt(state: State<'_, ExitGuardState>) {
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

        let tunnels = QuitBlockers { tunnels: 2 };
        assert!(state.should_hold(&tunnels), "first attempt asks the user");
        assert!(!state.should_hold(&tunnels), "insisting lets the app quit");

        state.prompt_open.store(false, Ordering::SeqCst);
        assert!(state.should_hold(&tunnels), "cancelling re-arms the prompt");

        state.confirmed.store(true, Ordering::SeqCst);
        assert!(!state.should_hold(&tunnels), "confirmed");
    }
}
