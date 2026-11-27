//! Startup settings and the values the workspace root is built from.

use std::rc::Rc;

use gpui_kit::Entity;
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::config::app_config_normalize::normalize_loaded_config;
use tterm_core::config::load_config_file;
use tterm_core::events::AppEvent;
use tterm_terminal_view::input::keyboard::EncoderConfig;

use super::super::command::CommandSpec;
use super::super::keymap::KeymapRouter;
use super::session_workspace::StartupWorkspace;
use super::{ConfigState, Locale, SessionManager};
use crate::app_window::theme::ResolvedTheme;

pub(super) struct PrototypeStartup {
    pub(super) command: CommandSpec,
    pub(super) workspace: StartupWorkspace,
    pub(super) sessions: Rc<SessionManager>,
    pub(super) events: UnboundedReceiver<AppEvent>,
    pub(super) keyboard: EncoderConfig,
    pub(super) keymap: Rc<KeymapRouter>,
    pub(super) locale: Entity<Locale>,
    pub(super) config: Entity<ConfigState>,
    pub(super) theme: ResolvedTheme,
}

/// Loads settings with the same sequence the Tauri client uses: the strict
/// file reader, then the shared UI normalization before any view reads them.
pub(super) fn load_startup_config() -> AppConfig {
    startup_config(load_config_file())
}

/// Normalizes a loaded config, or starts from the normalized defaults when
/// the strict reader rejects the file. The Tauri frontend makes the same
/// fallback in `ConfigContext`.
pub(super) fn startup_config(loaded: Result<AppConfig, String>) -> AppConfig {
    let normalized = loaded.and_then(|config| {
        normalize_loaded_config(&config)
            .map_err(|error| format!("Failed to normalize config: {error}"))
    });
    normalized.unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to load config, using defaults: {error}");
        default_startup_config()
    })
}

fn default_startup_config() -> AppConfig {
    match normalize_loaded_config(&AppConfig::default()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("tterm-gpui: failed to normalize the default config: {error}");
            AppConfig::default()
        }
    }
}
