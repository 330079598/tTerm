//! The Tauri command listing the fonts installed on this computer.

#[tauri::command]
pub async fn list_fonts() -> Vec<String> {
    tokio::task::spawn_blocking(super::list_fonts_sync)
        .await
        .unwrap_or_default()
}
