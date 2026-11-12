//! Tauri commands the UI uses while the app waits to quit.

use std::sync::Arc;

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
