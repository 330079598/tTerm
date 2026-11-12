//! Closing the main window either quits or keeps tTerm running in the
//! background behind a tray icon (the menu bar on macOS), as the user chose in
//! `close_behavior`. "ask" lets the UI decide on each close.

use std::sync::Arc;
use tterm_core::events::background::BackgroundEvent;
use tterm_core::events::{AppEvent, EventSink};

pub(crate) mod exit_guard;
pub(crate) mod quit;
pub(crate) mod tray;

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseBehavior {
    Ask = 0,
    Tray = 1,
    Quit = 2,
}

impl CloseBehavior {
    pub fn parse(value: &str) -> Self {
        match value {
            "tray" => Self::Tray,
            "quit" => Self::Quit,
            _ => Self::Ask,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Tray,
            2 => Self::Quit,
            _ => Self::Ask,
        }
    }
}

/// The current `close_behavior`, kept in memory so closing the window never
/// waits on the config file.
pub struct CloseBehaviorState {
    behavior: AtomicU8,
    prompt_open: AtomicBool,
}

impl Default for CloseBehaviorState {
    fn default() -> Self {
        Self {
            behavior: AtomicU8::new(CloseBehavior::Ask as u8),
            prompt_open: AtomicBool::new(false),
        }
    }
}

impl CloseBehaviorState {
    fn get(&self) -> CloseBehavior {
        CloseBehavior::from_u8(self.behavior.load(Ordering::SeqCst))
    }

    fn set(&self, behavior: CloseBehavior) {
        self.behavior.store(behavior as u8, Ordering::SeqCst);
    }

    /// `true` when the UI should be asked. Closing again while it is asking
    /// means the user insists (or the UI cannot answer); the window then
    /// closes as if "quit" had been chosen, so nobody is trapped in the app.
    fn should_ask(&self) -> bool {
        !self.prompt_open.swap(true, Ordering::SeqCst)
    }
}

/// Concrete dependencies for close and exit policy.
#[derive(Clone)]
pub(crate) struct BackgroundContext {
    pub close: Arc<CloseBehaviorState>,
    pub guard: Arc<exit_guard::ExitGuardState>,
    pub tunnels: Arc<crate::tunnel::manager::TunnelManager>,
    pub unuploaded_files: Vec<String>,
    pub window: Arc<dyn tterm_platform::window::MainWindow>,
    pub badge: Arc<dyn tterm_platform::notifications::AppBadge>,
    pub events: Arc<dyn EventSink>,
    pub quit: quit::QuitContext,
    pub tray: tray::TrayContext,
}

/// Applies the saved close behavior and updates the tray.
pub(crate) fn apply_config(state: &CloseBehaviorState, tray: &tray::TrayContext, cfg: &AppConfig) {
    let behavior = CloseBehavior::parse(&cfg.close_behavior);
    state.set(behavior);
    let result = if behavior == CloseBehavior::Quit {
        tray.native.remove()
    } else {
        tray::show(tray, &cfg.language)
    };
    if let Err(error) = result {
        eprintln!("Failed to update the tray icon: {error}");
    }
}

/// Brings the main window back and restores its taskbar overlay.
pub(crate) fn show_main_window(
    window: &dyn tterm_platform::window::MainWindow,
    badge: &dyn tterm_platform::notifications::AppBadge,
) {
    let _ = window.show_and_focus();
    crate::notification::badge::restore(badge);
}

pub(crate) fn request_quit(context: &BackgroundContext) {
    if !exit_guard::blockers(&context.tunnels, &context.unuploaded_files).is_empty() {
        show_main_window(context.window.as_ref(), context.badge.as_ref());
    }
    quit_unless_held(context);
}

fn send_to_background(context: &BackgroundContext) {
    let _ = context
        .window
        .send_to_background(context.tray.native.is_present());
}

/// Returns whether the native close must be prevented.
pub(crate) fn on_close_requested(context: &BackgroundContext) -> bool {
    match context.close.get() {
        CloseBehavior::Tray => send_to_background(context),
        CloseBehavior::Ask if context.close.should_ask() => {
            if !emit_close_request(&context.close, context.events.as_ref()) {
                quit_unless_held(context);
            }
        }
        CloseBehavior::Ask | CloseBehavior::Quit => quit_unless_held(context),
    }
    true
}

fn emit_close_request(state: &CloseBehaviorState, events: &dyn EventSink) -> bool {
    if events
        .emit(AppEvent::Background(BackgroundEvent::CloseRequested))
        .is_err()
    {
        state.prompt_open.store(false, Ordering::SeqCst);
        return false;
    }
    true
}

/// Quits, unless what the quit would end holds the exit for the user's
/// confirmation. The window stays open meanwhile: the UI saves before the
/// quit closes it.
fn quit_unless_held(context: &BackgroundContext) {
    if !exit_guard::hold_exit_for_confirmation(
        &context.guard,
        exit_guard::blockers(&context.tunnels, &context.unuploaded_files),
        context.events.as_ref(),
    ) {
        quit::quit(&context.quit);
    }
}

/// Returns whether an external exit request must wait for confirmation or saving.
pub(crate) fn on_exit_requested(context: &BackgroundContext) -> bool {
    exit_guard::hold_exit_for_confirmation(
        &context.guard,
        exit_guard::blockers(&context.tunnels, &context.unuploaded_files),
        context.events.as_ref(),
    ) || quit::quit_after_save(&context.quit)
}

/// Resolves the UI's close choice using the same close policy as native events.
#[tauri::command]
pub fn resolve_close_request(
    action: String,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) {
    resolve_close(
        &crate::app_context::background(
            &services,
            crate::sftp::internal::api::external::unuploaded_files(&services.sftp.external_edit),
        ),
        &action,
    );
}

fn resolve_close(context: &BackgroundContext, action: &str) {
    context.close.prompt_open.store(false, Ordering::SeqCst);
    match action {
        "tray" => send_to_background(context),
        "quit" => request_quit(context),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_close_behavior_asks() {
        assert_eq!(CloseBehavior::parse("tray"), CloseBehavior::Tray);
        assert_eq!(CloseBehavior::parse("quit"), CloseBehavior::Quit);
        assert_eq!(CloseBehavior::parse("ask"), CloseBehavior::Ask);
        assert_eq!(CloseBehavior::parse(""), CloseBehavior::Ask);
    }

    #[test]
    fn closing_again_while_asking_goes_ahead() {
        let state = CloseBehaviorState::default();
        assert!(state.should_ask(), "first close asks");
        assert!(!state.should_ask(), "insisting skips the prompt");
        state.prompt_open.store(false, Ordering::SeqCst);
        assert!(state.should_ask(), "an answer re-arms the prompt");
    }

    #[test]
    fn behavior_round_trips_through_the_state() {
        let state = CloseBehaviorState::default();
        assert_eq!(state.get(), CloseBehavior::Ask);
        for behavior in [CloseBehavior::Tray, CloseBehavior::Quit, CloseBehavior::Ask] {
            state.set(behavior);
            assert_eq!(state.get(), behavior);
        }
    }
}

#[cfg(test)]
#[path = "close_order_tests.rs"]
mod close_order_tests;
