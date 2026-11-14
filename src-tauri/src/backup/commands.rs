//! The Tauri commands for local backup files.

use crate::{
    backup::AutomaticBackupSettings, backup::BackupExportOptions, backup::BackupExportResult,
    backup::BackupHistoryEntry, backup::BackupImportOptions, backup::BackupImportResult,
    backup::BackupInspectInput, backup::BackupInspectResult,
};
use serde_json::Value;

#[tauri::command]
pub async fn export_backup(
    output_path: String,
    options: BackupExportOptions,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupExportResult, String> {
    let app_version = services.shared.identity.version.clone();
    super::service::export_backup(app_version, output_path, options, &services.shared.secrets).await
}

#[tauri::command]
pub async fn inspect_backup(
    input: BackupInspectInput,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupInspectResult, String> {
    super::service::inspect_backup(input, &services.shared.secrets).await
}

#[tauri::command]
pub async fn import_backup(
    input_path: String,
    options: BackupImportOptions,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupImportResult, String> {
    let app_version = services.shared.identity.version.clone();
    super::service::import_backup(app_version, input_path, options, &services.shared.secrets).await
}

#[tauri::command]
pub async fn run_due_automatic_backup(
    frontend_state: Option<Value>,
    force: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Option<BackupExportResult>, String> {
    let app_version = services.shared.identity.version.clone();
    super::service::run_due_automatic_backup(
        app_version,
        frontend_state,
        force,
        &services.shared.secrets,
    )
    .await
}

#[tauri::command]
pub async fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    super::service::get_automatic_backup_settings().await
}

#[tauri::command]
pub async fn save_automatic_backup_settings(
    settings: AutomaticBackupSettings,
) -> Result<AutomaticBackupSettings, String> {
    super::service::save_automatic_backup_settings(settings).await
}

#[tauri::command]
pub async fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
    super::service::list_backup_history().await
}

#[tauri::command]
pub async fn delete_backup_history_entry(path: String) -> Result<bool, String> {
    super::service::delete_backup_history_entry(path).await
}
