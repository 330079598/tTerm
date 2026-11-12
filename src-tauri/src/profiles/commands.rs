use super::service;
use super::types::{
    SavedProfile, SshConfigImportHost, SshConfigImportOptions, SshConfigImportPreview,
    SshConfigImportResult, TestConnectionResult,
};
use crate::config::tab_icons::TabIconStyle;
use crate::services::AppServices;
use std::sync::Arc;

pub(super) mod connection_check;
pub(crate) mod groups;
pub(super) mod import;

#[tauri::command]
pub async fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    service::list_profiles().await
}

#[tauri::command]
pub async fn preview_ssh_config_import(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    service::preview_ssh_config_import(source_path).await
}

#[tauri::command]
pub async fn resolve_ssh_config_host(name: String) -> Result<Option<SshConfigImportHost>, String> {
    service::resolve_ssh_config_host(name).await
}

#[tauri::command]
pub async fn import_ssh_config_profiles(
    options: SshConfigImportOptions,
) -> Result<SshConfigImportResult, String> {
    service::import_ssh_config_profiles(options).await
}

#[tauri::command]
pub async fn save_profile(
    services: tauri::State<'_, Arc<AppServices>>,
    profile: SavedProfile,
    typed_password_tab_id: Option<String>,
) -> Result<(), String> {
    service::save_profile(&services.shared.secrets, profile, typed_password_tab_id).await
}

#[tauri::command]
pub async fn delete_profile(
    services: tauri::State<'_, Arc<AppServices>>,
    id: String,
) -> Result<(), String> {
    service::delete_profile(&services.shared.secrets, &services.app.tunnels, id).await
}

#[tauri::command]
pub async fn delete_profiles(
    services: tauri::State<'_, Arc<AppServices>>,
    ids: Vec<String>,
) -> Result<(), String> {
    service::delete_profiles(&services.shared.secrets, &services.app.tunnels, ids).await
}

#[tauri::command]
pub async fn set_profile_server_monitor_visible(id: String, visible: bool) -> Result<(), String> {
    service::set_profile_server_monitor_visible(id, visible).await
}

#[tauri::command]
pub async fn set_profile_distro(id: String, distro: Option<String>) -> Result<(), String> {
    service::set_profile_distro(id, distro).await
}

#[tauri::command]
pub async fn set_profile_tab_icon(id: String, icon: Option<TabIconStyle>) -> Result<(), String> {
    service::set_profile_tab_icon(id, icon).await
}

#[tauri::command]
pub async fn test_connection(
    services: tauri::State<'_, Arc<AppServices>>,
    profile: SavedProfile,
    typed_password_tab_id: Option<String>,
) -> Result<TestConnectionResult, String> {
    service::test_connection(
        crate::connection_context::connection(&services),
        profile,
        typed_password_tab_id,
        &services.shared.secrets,
    )
    .await
}
