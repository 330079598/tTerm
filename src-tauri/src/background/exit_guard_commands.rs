//! Tauri commands that answer the quit-confirmation prompt.

use std::sync::Arc;

#[tauri::command]
pub fn confirm_quit_app(services: tauri::State<'_, Arc<tterm_core::services::AppServices>>) {
    tterm_core::background::exit_guard::confirm(
        &services.app.exit_guard,
        &crate::app_context::quit(&services),
    );
}

#[tauri::command]
pub fn cancel_quit_prompt(services: tauri::State<'_, Arc<tterm_core::services::AppServices>>) {
    services.app.exit_guard.cancel();
}
