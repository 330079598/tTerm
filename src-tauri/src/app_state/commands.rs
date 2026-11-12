//! The Tauri commands for the state tTerm keeps for itself.

use super::AppState;

#[tauri::command]
pub async fn load_app_state() -> Result<AppState, String> {
    crate::core::blocking::run_blocking(|| tterm_core::db::read(super::load)).await
}

#[tauri::command]
pub async fn save_app_state(state: AppState) -> Result<(), String> {
    crate::core::blocking::run_blocking(move || {
        tterm_core::db::write(|transaction| super::save(transaction, &state))
    })
    .await
}
