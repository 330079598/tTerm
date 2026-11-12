//! The Tauri commands for custom themes.

use serde_json::Value;

/// The saved themes. On the first call, `legacy` (the list the web view
/// kept in localStorage) is taken over when given.
#[tauri::command]
pub async fn load_custom_themes(legacy: Option<Vec<Value>>) -> Result<Vec<Value>, String> {
    tterm_core::themes::load(legacy).await
}

#[tauri::command]
pub async fn save_custom_themes(themes: Vec<Value>) -> Result<(), String> {
    tterm_core::themes::save(themes).await
}
