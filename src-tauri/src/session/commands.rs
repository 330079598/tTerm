//! The Tauri commands for the saved session.

use super::SessionData;

#[tauri::command]
pub async fn load_session() -> Result<SessionData, String> {
    super::load_session_service().await
}

#[tauri::command]
pub async fn save_session(session: SessionData) -> Result<(), String> {
    super::save_session_service(session).await
}

#[tauri::command]
pub async fn clear_session() -> Result<(), String> {
    super::clear_session_service().await
}
