//! Closing the main window either quits or keeps tTerm running in the
//! background behind a tray icon (the menu bar on macOS), as the user chose in
//! `close_behavior`. "ask" lets the UI decide on each close.

mod tray;

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tauri::{AppHandle, Emitter, Manager, State, Window};

use crate::config::AppConfig;
use crate::tunnel::TunnelManager;

pub use tray::{refresh_tunnels, TrayState};

const CLOSE_REQUESTED_EVENT: &str = "close-requested";

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

/// Applies a saved config: remembers its close behavior and adds, updates or
/// removes the tray icon to match.
pub fn apply_config(app: &AppHandle, cfg: &AppConfig) {
    let behavior = CloseBehavior::parse(&cfg.close_behavior);
    app.state::<CloseBehaviorState>().set(behavior);
    if let Err(error) = sync_tray(app, behavior, &cfg.language) {
        eprintln!("Failed to update the tray icon: {error}");
    }
}

fn sync_tray(app: &AppHandle, behavior: CloseBehavior, language: &str) -> tauri::Result<()> {
    if behavior == CloseBehavior::Quit {
        tray::hide(app);
        return Ok(());
    }
    tray::show(app, language)
}

/// Brings the main window back, whether it is hidden, minimized or behind
/// other windows.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Quits the way the window would: running tunnels are confirmed first, in
/// the window, so it is shown before asking.
fn request_quit(app: &AppHandle) {
    let tunnels = app.state::<TunnelManager>();
    if tunnels.has_active() {
        show_main_window(app);
    }
    if !tunnels.hold_exit_for_confirmation(app) {
        app.exit(0);
    }
}

/// Keeps the app running without its window. Without a tray icon (it could
/// not be created, e.g. no tray on this Linux desktop) the window is only
/// minimized so it can still be found.
fn send_to_background(window: &Window) {
    if window.app_handle().tray_by_id(tray::TRAY_ID).is_some() {
        let _ = window.hide();
    } else {
        let _ = window.minimize();
    }
}

/// Shows the window when something running in the background needs an
/// answer from the user (a host key or a login prompt). Callable from any
/// thread.
pub fn reveal_for_prompt(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = handle.get_webview_window("main") else {
            return;
        };
        let hidden = !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false);
        if hidden {
            show_main_window(&handle);
        }
    });
}

/// Handles a request to close the main window. Returns `true` when the close
/// must be prevented.
pub fn on_close_requested(window: &Window) -> bool {
    let app = window.app_handle();
    let state = app.state::<CloseBehaviorState>();
    match state.get() {
        CloseBehavior::Tray => {
            send_to_background(window);
            true
        }
        CloseBehavior::Ask if state.should_ask() => {
            if app.emit(CLOSE_REQUESTED_EVENT, ()).is_err() {
                state.prompt_open.store(false, Ordering::SeqCst);
                return app.state::<TunnelManager>().hold_exit_for_confirmation(app);
            }
            true
        }
        CloseBehavior::Ask | CloseBehavior::Quit => {
            app.state::<TunnelManager>().hold_exit_for_confirmation(app)
        }
    }
}

/// The UI's answer to `close-requested`: "tray", "quit" or "cancel". A
/// remembered choice is saved by the UI through `save_config` beforehand.
#[tauri::command]
pub fn resolve_close_request(
    app: AppHandle,
    window: Window,
    action: String,
    state: State<'_, CloseBehaviorState>,
) {
    state.prompt_open.store(false, Ordering::SeqCst);
    match action.as_str() {
        "tray" => send_to_background(&window),
        "quit" => request_quit(&app),
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
