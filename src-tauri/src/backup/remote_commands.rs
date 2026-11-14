//! The Tauri commands for backups kept on a WebDAV server.

use serde_json::Value;
use tterm_core::backup::remote::RemoteBackupEntry;
use tterm_core::backup::remote::WebDavBackupSettings;
use tterm_core::backup::remote::WebDavBackupStatus;
use tterm_core::backup::remote::WebDavDeleteResult;
use tterm_core::backup::remote::WebDavUploadResult;

#[tauri::command]
pub async fn get_webdav_backup_status(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<WebDavBackupStatus, String> {
    tterm_core::backup::remote::service::get_webdav_backup_status(&services.shared.secrets).await
}

/// Saves the settings. A password left `None` or empty keeps the saved one.
#[tauri::command]
pub async fn save_webdav_backup_settings(
    settings: WebDavBackupSettings,
    password: Option<String>,
    backup_password: Option<String>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<WebDavBackupStatus, String> {
    tterm_core::backup::remote::service::save_webdav_backup_settings(
        settings,
        password,
        backup_password,
        &services.shared.secrets,
    )
    .await
}

/// Forgets the server, its password and the backup password. Remote files stay.
#[tauri::command]
pub async fn clear_webdav_backup_settings(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<WebDavBackupStatus, String> {
    tterm_core::backup::remote::service::clear_webdav_backup_settings(&services.shared.secrets)
        .await
}

/// Checks the server and credentials and creates the remote folder. Uses
/// the entered password when given, else the saved one.
#[tauri::command]
pub async fn test_webdav_connection(
    settings: WebDavBackupSettings,
    password: Option<String>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::test_webdav_connection(
        app_version,
        settings,
        password,
        &services.shared.secrets,
    )
    .await
}

/// Uploads a backup when one is due, or now when `force` is set. A scheduled
/// run that cannot start (not configured, passwords locked, another upload
/// running) returns `None` and tries again next time.
#[tauri::command]
pub async fn run_webdav_backup(
    frontend_state: Option<Value>,
    force: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Option<WebDavUploadResult>, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::run_webdav_backup(
        app_version,
        frontend_state,
        force,
        &services.shared.secrets,
    )
    .await
}

#[tauri::command]
pub async fn list_webdav_backups(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Vec<RemoteBackupEntry>, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::list_webdav_backups(app_version, &services.shared.secrets)
        .await
}

/// Downloads a remote backup next to the config so the import flow can
/// inspect it like a local file. Returns the local path.
#[tauri::command]
pub async fn download_webdav_backup(
    file_name: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<String, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::download_webdav_backup(
        app_version,
        file_name,
        &services.shared.secrets,
    )
    .await
}

/// Deletes the downloaded backup once the import view lets go of it.
#[tauri::command]
pub async fn discard_webdav_download() -> Result<(), String> {
    tterm_core::backup::remote::service::discard_webdav_download().await
}

#[tauri::command]
pub async fn delete_webdav_backup(
    file_name: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::delete_webdav_backup(
        app_version,
        file_name,
        &services.shared.secrets,
    )
    .await
}

/// Deletes several remote backups, carrying on past a file that fails.
#[tauri::command]
pub async fn delete_webdav_backups(
    file_names: Vec<String>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<WebDavDeleteResult, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::delete_webdav_backups(
        app_version,
        file_names,
        &services.shared.secrets,
    )
    .await
}

/// Saves a remote backup to a path the user picked. Returns that path.
#[tauri::command]
pub async fn save_webdav_backup(
    file_name: String,
    output_path: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<String, String> {
    let app_version = services.shared.identity.version.clone();
    tterm_core::backup::remote::service::save_webdav_backup(
        app_version,
        file_name,
        output_path,
        &services.shared.secrets,
    )
    .await
}
