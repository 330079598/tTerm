//! The Tauri command for handing a tray update check to the UI.

use std::sync::Arc;

/// Hands the UI an update check asked for from the tray, once.
#[tauri::command]
pub fn take_tray_update_check(
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> bool {
    services.app.tray.take_update_check()
}
