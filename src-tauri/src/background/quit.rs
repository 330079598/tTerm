//! Quitting gives the UI a moment to save what it keeps only in the webview,
//! such as the terminals' content, which `app.exit` would otherwise end with
//! the window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

/// Told to the UI when the app is about to quit; it answers with
/// `ready_to_quit` once it has saved.
const WILL_QUIT_EVENT: &str = "app-will-quit";
/// How long a quit waits for the UI. A webview that hangs or is gone must
/// not keep the app from quitting.
const SAVE_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct QuitState {
    /// Whether the UI has something to save before a quit; when it has not,
    /// a quit does not wait at all.
    save_before_quit: AtomicBool,
    quitting: AtomicBool,
    saved: Notify,
}

impl QuitState {
    /// Whether this quit should wait for the UI. A second quit while the first
    /// still waits is the user insisting, and goes ahead at once.
    fn should_wait(&self) -> bool {
        self.save_before_quit.load(Ordering::SeqCst) && !self.quitting.swap(true, Ordering::SeqCst)
    }
}

/// Quits the app once the UI has saved, or once it took too long to.
pub fn quit(app: &AppHandle) {
    if !quit_after_save(app) {
        app.exit(0);
    }
}

/// Starts a quit that waits for the UI to save. Returns `false` when there
/// is nothing to wait for, and the caller is to exit at once.
pub fn quit_after_save(app: &AppHandle) -> bool {
    let state = app.state::<QuitState>();
    if !state.should_wait() || app.emit(WILL_QUIT_EVENT, ()).is_err() {
        return false;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<QuitState>();
        let _ = tokio::time::timeout(SAVE_TIMEOUT, state.saved.notified()).await;
        app.exit(0);
    });
    true
}

/// The UI tells whether it has something to save before a quit.
#[tauri::command]
pub fn set_save_before_quit(enabled: bool, state: State<'_, QuitState>) {
    state.save_before_quit.store(enabled, Ordering::SeqCst);
}

/// The UI's answer to `app-will-quit`: it has saved, and the app may quit.
#[tauri::command]
pub fn ready_to_quit(state: State<'_, QuitState>) {
    // A permit is kept when the quit is not waiting yet, so an answer that
    // comes first is not lost.
    state.saved.notify_one();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quit_waits_only_when_the_ui_has_something_to_save() {
        let state = QuitState::default();
        assert!(!state.should_wait());
        state.save_before_quit.store(true, Ordering::SeqCst);
        assert!(state.should_wait());
    }

    #[test]
    fn quitting_again_while_waiting_goes_ahead() {
        let state = QuitState::default();
        state.save_before_quit.store(true, Ordering::SeqCst);
        assert!(state.should_wait());
        assert!(!state.should_wait());
    }
}
