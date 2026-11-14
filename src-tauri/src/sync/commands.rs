//! The Tauri commands for syncing data between devices.

use crate::{sync::SyncOutcome, sync::SyncSettings, sync::SyncStatus};
use serde_json::Value;

#[tauri::command]
pub async fn get_sync_status() -> Result<SyncStatus, String> {
    super::service::get_sync_status().await
}

#[tauri::command]
pub async fn save_sync_settings(settings: SyncSettings) -> Result<SyncStatus, String> {
    super::service::save_sync_settings(settings).await
}

/// Forgets what was last synced, so the next sync merges like a first one:
/// both sides' data is kept and nothing is deleted.
#[tauri::command]
pub async fn reset_sync() -> Result<SyncStatus, String> {
    super::service::reset_sync().await
}

/// Syncs when this device has changes, or always when `check_remote` is
/// set. `frontend_state` is what the web view keeps, for the recovery backup
/// made before a sync deletes anything.
#[tauri::command]
pub async fn run_sync(
    frontend_state: Option<Value>,
    check_remote: bool,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SyncOutcome, String> {
    let app_version = services.shared.identity.version.clone();
    super::service::run_sync(
        app_version,
        frontend_state,
        check_remote,
        &services.shared.secrets,
        &services.app.tunnels,
    )
    .await
}
