//! The Tauri command for saving the last window background.

#[tauri::command]
pub async fn save_window_background(color: String) -> Result<(), String> {
    crate::core::blocking::run_blocking(move || {
        tterm_core::db::write(|transaction| super::window_background::write(transaction, &color))
    })
    .await
}
