//! Tauri update commands and the channel bridge for download progress.

use std::sync::Arc;

use tauri::ipc::Channel;

use super::{
    AppUpdateDownloadEvent, AppUpdateMetadata, Progress, check_app_update_service,
    download_app_update_service, download_install_app_update_service,
    install_downloaded_app_update_service,
};

#[tauri::command]
pub async fn check_app_update(
    channel: String,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<Option<AppUpdateMetadata>, String> {
    check_app_update_service(services.shared.platform.updates.as_ref(), channel).await
}

#[tauri::command]
pub async fn download_app_update(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
    app: tauri::AppHandle,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let progress = Arc::new(Progress(Arc::new(move |event| {
        let _ = on_event.send(event);
    })));
    download_app_update_service(
        services.shared.platform.updates.as_ref(),
        &services.app.pending_updates,
        crate::app_paths::get(&app)?.as_ref().clone(),
        channel,
        progress,
    )
    .await
}

#[tauri::command]
pub async fn install_downloaded_app_update(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
    channel: String,
) -> Result<bool, String> {
    install_downloaded_app_update_service(
        services.shared.platform.main_window.as_ref(),
        &services.app.pending_updates,
        channel,
    )
    .await
}

#[tauri::command]
pub async fn download_install_app_update(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let progress = Arc::new(Progress(Arc::new(move |event| {
        let _ = on_event.send(event);
    })));
    download_install_app_update_service(
        services.shared.platform.updates.as_ref(),
        services.shared.platform.main_window.as_ref(),
        &services.app.pending_updates,
        channel,
        progress,
    )
    .await
}
