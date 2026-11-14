//! The Tauri commands for local backup files.

use serde_json::Value;
use tterm_core::backup::AutomaticBackupSettings;
use tterm_core::backup::BackupExportOptions;
use tterm_core::backup::BackupExportResult;
use tterm_core::backup::BackupHistoryEntry;
use tterm_core::backup::BackupImportOptions;
use tterm_core::backup::BackupImportResult;
use tterm_core::backup::BackupInspectInput;
use tterm_core::backup::BackupInspectResult;

#[tauri::command]
pub async fn export_backup(
    output_path: String,
    options: BackupExportOptions,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupExportResult, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::service::export_backup(
        app_version,
        output_path,
        options,
        &services.shared.secrets,
    )
    .await
}

#[tauri::command]
pub async fn inspect_backup(
    input: BackupInspectInput,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupInspectResult, String> {
    tterm_core::backup::service::inspect_backup(input, &services.shared.secrets).await
}

#[tauri::command]
pub async fn import_backup(
    input_path: String,
    options: BackupImportOptions,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<BackupImportResult, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::service::import_backup(
        app_version,
        input_path,
        options,
        &services.shared.secrets,
    )
    .await
}

#[tauri::command]
pub async fn run_due_automatic_backup(
    frontend_state: Option<Value>,
    force: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Option<BackupExportResult>, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::service::run_due_automatic_backup(
        app_version,
        frontend_state,
        force,
        &services.shared.secrets,
    )
    .await
}

#[tauri::command]
pub async fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    tterm_core::backup::service::get_automatic_backup_settings().await
}

#[tauri::command]
pub async fn save_automatic_backup_settings(
    settings: AutomaticBackupSettings,
) -> Result<AutomaticBackupSettings, String> {
    tterm_core::backup::service::save_automatic_backup_settings(settings).await
}

#[tauri::command]
pub async fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
    tterm_core::backup::service::list_backup_history().await
}

#[tauri::command]
pub async fn delete_backup_history_entry(path: String) -> Result<bool, String> {
    tterm_core::backup::service::delete_backup_history_entry(path).await
}
