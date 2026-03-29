use crate::config::get_config_path;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionData {
    pub tabs: serde_json::Value,
    pub active_tab_id: Option<String>,
    pub last_saved: i64,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: serde_json::json!([]),
            active_tab_id: None,
            last_saved: 0,
        }
    }
}

#[tauri::command]
pub fn load_session() -> Result<SessionData, String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if !session_file.exists() {
        return Ok(SessionData::default());
    }
    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {}", e))
}

#[tauri::command]
pub fn save_session(session: SessionData) -> Result<(), String> {
    let config_dir = crate::config::ensure_config_dir()?;
    let session_file = config_dir.join("session.json");
    let content = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    fs::write(&session_file, content).map_err(|e| format!("Failed to write session file: {}", e))
}

#[tauri::command]
pub fn clear_session() -> Result<(), String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if session_file.exists() {
        fs::remove_file(&session_file)
            .map_err(|e| format!("Failed to remove session file: {}", e))?;
    }
    Ok(())
}
