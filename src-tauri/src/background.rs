//! Closing the main window either quits or keeps tTerm running in the
//! background behind a tray icon (the menu bar on macOS), as the user chose in
//! `close_behavior`. "ask" lets the UI decide on each close.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Once;

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, Window};

use crate::config::AppConfig;
use crate::tunnel::TunnelManager;

const TRAY_ID: &str = "main";
const MENU_SHOW: &str = "tray-show";
const MENU_QUIT: &str = "tray-quit";
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
        app.remove_tray_by_id(TRAY_ID);
        return Ok(());
    }
    // A tray's own menu handler outlives the tray, so one built again after
    // being removed would answer each click twice; this one is app-wide.
    static MENU_HANDLER: Once = Once::new();
    MENU_HANDLER.call_once(|| app.on_menu_event(on_menu_event));
    let menu = tray_menu(app, language)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        return tray.set_menu(Some(menu));
    }
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("tTerm")
        .menu(&menu)
        // macOS opens the menu on a click, like every menu bar item; elsewhere
        // a click brings the window back and the menu is on right click.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_tray_icon_event(on_tray_icon_event);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn tray_menu(app: &AppHandle, language: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let (show, quit) = if language == "zh" {
        ("显示 tTerm", "退出")
    } else {
        ("Show tTerm", "Quit")
    };
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, MENU_SHOW, show, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, MENU_QUIT, quit, true, None::<&str>)?,
        ],
    )
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => show_main_window(app),
        MENU_QUIT => request_quit(app),
        _ => {}
    }
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    let show = match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => !cfg!(target_os = "macos"),
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => true,
        _ => false,
    };
    if show {
        show_main_window(tray.app_handle());
    }
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
    if window.app_handle().tray_by_id(TRAY_ID).is_some() {
        let _ = window.hide();
    } else {
        let _ = window.minimize();
    }
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
