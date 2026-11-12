//! The Tauri commands for system notifications and their sound.

use super::{NotificationPermission, NotificationRequest};
use std::sync::Arc;
use tauri::ipc::Response;

/// Resolves once the notification is shown, or with why it was not (macOS
/// reports a refusal; elsewhere it resolves once handed to the system).
#[tauri::command]
pub async fn show_notification(
    request: NotificationRequest,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    super::show(services.shared.platform.notifications.as_ref(), request).await
}

#[tauri::command]
pub async fn notification_permission(app: tauri::AppHandle) -> NotificationPermission {
    use tauri::Manager;
    let Some(services) = app.try_state::<Arc<crate::services::AppServices>>() else {
        return NotificationPermission::NotDetermined;
    };
    let notifications = services.shared.platform.notifications.clone();
    super::permission(notifications.as_ref()).await
}

/// Copies the audio file at `path` in as the notification sound and returns
/// its name. tTerm plays the sound itself because a Windows toast plays only
/// the system's sounds for an app that is not packaged.
#[tauri::command]
pub async fn import_notification_sound(path: String) -> Result<String, String> {
    tterm_core::core::blocking::run_blocking(move || {
        super::sound::import(std::path::Path::new(&path))
    })
    .await
}

/// The notification sound's bytes, which the frontend decodes and plays.
#[tauri::command]
pub async fn read_notification_sound() -> Result<Response, String> {
    tterm_core::core::blocking::run_blocking(super::sound::read)
        .await
        .map(Response::new)
}
