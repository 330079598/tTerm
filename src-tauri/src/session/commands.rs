//! The Tauri commands for the saved session.

use tterm_core::session_restore::SessionData;

#[tauri::command]
pub async fn load_session() -> Result<SessionData, String> {
    tterm_core::session_restore::load_session_service().await
}

#[tauri::command]
pub async fn save_session(session: SessionData) -> Result<(), String> {
    tterm_core::session_restore::save_session_service(session).await
}

#[tauri::command]
pub async fn clear_session() -> Result<(), String> {
    tterm_core::session_restore::clear_session_service().await
}
