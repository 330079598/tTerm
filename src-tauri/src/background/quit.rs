//! Quitting gives the UI a moment to save what it keeps only in the webview,
//! such as the terminals' content, which `app.exit` would otherwise end with
//! the window.

use std::sync::Arc;
use tterm_core::events::background::BackgroundEvent;
use tterm_core::events::{AppEvent, EventSink};

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Notify;
use tterm_platform::process::ProcessControl;

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
    fn set_save_before_quit(&self, enabled: bool) {
        self.save_before_quit.store(enabled, Ordering::SeqCst);
    }

    fn ready(&self) {
        self.saved.notify_one();
    }

    /// Whether this quit should wait for the UI. A second quit while the first
    /// still waits is the user insisting, and goes ahead at once.
    fn should_wait(&self) -> bool {
        self.save_before_quit.load(Ordering::SeqCst) && !self.quitting.swap(true, Ordering::SeqCst)
    }
}

/// Dependencies retained only until the final exit request.
#[derive(Clone)]
pub(crate) struct QuitContext {
    pub state: Arc<QuitState>,
    pub events: Arc<dyn EventSink>,
    pub process: Arc<dyn ProcessControl>,
    pub runtime: tokio::runtime::Handle,
}

/// Quits the app once the UI has saved, or once it took too long to.
pub(crate) fn quit(context: &QuitContext) {
    if !quit_after_save(context)
        && let Err(error) = context.process.exit(0) {
            eprintln!("Failed to exit tTerm: {error}");
        }
}

/// Returns false when the caller should exit immediately.
pub(crate) fn quit_after_save(context: &QuitContext) -> bool {
    if !begin_save(&context.state, context.events.as_ref()) {
        return false;
    }
    let context = context.clone();
    context.runtime.clone().spawn(async move {
        wait_for_save(&context.state).await;
        if let Err(error) = context.process.exit(0) {
            eprintln!("Failed to exit tTerm: {error}");
        }
    });
    true
}

fn begin_save(state: &QuitState, events: &dyn EventSink) -> bool {
    state.should_wait()
        && events
            .emit(AppEvent::Background(BackgroundEvent::WillQuit))
            .is_ok()
}

async fn wait_for_save(state: &QuitState) {
    let _ = tokio::time::timeout(SAVE_TIMEOUT, state.saved.notified()).await;
}

/// The UI tells whether it has something to save before a quit.
#[tauri::command]
pub fn set_save_before_quit(
    enabled: bool,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) {
    services.app.quit.set_save_before_quit(enabled);
}

/// The UI's answer to `app-will-quit`: it has saved, and the app may quit.
#[tauri::command]
pub fn ready_to_quit(services: tauri::State<'_, Arc<crate::services::AppServices>>) {
    // A permit is kept when the quit is not waiting yet, so an answer that
    // comes first is not lost.
    services.app.quit.ready();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingProcess(std::sync::atomic::AtomicUsize);
    impl tterm_platform::process::ProcessControl for RecordingProcess {
        fn exit(&self, code: i32) -> Result<(), String> {
            assert_eq!(code, 0);
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn restart(&self) -> Result<(), String> {
            panic!("quit must never restart");
        }
    }
    #[tokio::test]
    async fn final_exit_happens_after_ack_or_timeout_and_repeat_quit_is_not_trapped() {
        for answer in [true, false] {
            let process =
                std::sync::Arc::new(RecordingProcess(std::sync::atomic::AtomicUsize::new(0)));
            let state = Arc::new(QuitState::default());
            let context = QuitContext {
                state: state.clone(),
                events: Arc::new(tterm_core::events::recording::RecordingEventSink::default()),
                process: process.clone(),
                runtime: tokio::runtime::Handle::current(),
            };
            state.save_before_quit.store(true, Ordering::SeqCst);
            quit(&context);
            tokio::task::yield_now().await;
            assert_eq!(process.0.load(Ordering::SeqCst), 0);
            if answer {
                state.saved.notify_one();
            } else {
                tokio::time::sleep(SAVE_TIMEOUT + Duration::from_millis(100)).await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            assert_eq!(process.0.load(Ordering::SeqCst), 1);
            quit(&context);
            assert_eq!(process.0.load(Ordering::SeqCst), 2);
        }
    }

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

#[cfg(test)]
#[path = "quit_tests.rs"]
mod order_tests;
