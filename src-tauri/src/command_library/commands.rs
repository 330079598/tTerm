use super::{models::CommandVariable, models::SavedCommand, service};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCommandInput {
    pub id: Option<String>,
    pub name: String,
    pub command_text: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_scope_type")]
    pub scope_type: String,
    pub scope_id: Option<String>,
    #[serde(default)]
    pub is_favorite: bool,
    /// `None` keeps the stored value, so callers that predate the field do not reset it.
    #[serde(default)]
    pub confirm_before_run: Option<bool>,
    /// Placeholders (`{{name}}`) to ask for on insert. `None` keeps the stored ones.
    #[serde(default)]
    pub variables: Option<Vec<CommandVariable>>,
}

fn default_scope_type() -> String {
    "global".to_string()
}

#[tauri::command]
pub async fn list_saved_commands() -> Result<Vec<SavedCommand>, String> {
    service::list_saved_commands().await
}

#[tauri::command]
pub async fn list_command_tags() -> Result<Vec<String>, String> {
    service::list_command_tags().await
}

#[tauri::command]
pub async fn create_command_tag(tag: String) -> Result<String, String> {
    service::create_command_tag(tag).await
}

#[tauri::command]
pub async fn rename_command_tag(old_tag: String, new_tag: String) -> Result<Vec<String>, String> {
    service::rename_command_tag(old_tag, new_tag).await
}

#[tauri::command]
pub async fn delete_command_tag(tag: String) -> Result<Vec<String>, String> {
    service::delete_command_tag(tag).await
}

#[tauri::command]
pub async fn save_saved_command(input: SaveCommandInput) -> Result<SavedCommand, String> {
    service::save_saved_command(input).await
}

#[tauri::command]
pub async fn delete_saved_command(id: String) -> Result<bool, String> {
    service::delete_saved_command(id).await
}

#[tauri::command]
pub async fn set_saved_command_favorite(
    id: String,
    favorite: bool,
) -> Result<SavedCommand, String> {
    service::set_saved_command_favorite(id, favorite).await
}

#[tauri::command]
pub async fn record_saved_command_use(id: String) -> Result<(), String> {
    service::record_saved_command_use(id).await
}
