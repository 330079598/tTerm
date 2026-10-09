//! System notifications for terminal events: a long command finishing, a
//! notification a program asked for, a bell. Clicking one brings the window
//! forward on the tab that raised it.

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Sent to the frontend when a notification is clicked.
const ACTIVATED_EVENT: &str = "notification-activated";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRequest {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub body: String,
    /// The tab to show when the notification is clicked.
    #[serde(default)]
    pub tab_id: Option<String>,
    #[serde(default)]
    pub sound: bool,
}

/// Whether notifications reach the user.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationPermission {
    Granted,
    Denied,
    /// macOS asks the first time one is shown.
    NotDetermined,
    /// A development build outside an app bundle: shown through AppleScript,
    /// and clicking it does not lead back to the tab.
    Fallback,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivatedPayload {
    tab_id: Option<String>,
}

/// A notification was clicked: show the window and the tab it came from.
fn activated(app: &AppHandle, tab_id: Option<String>) {
    crate::background::show_main_window(app);
    if let Err(error) = app.emit(ACTIVATED_EVENT, ActivatedPayload { tab_id }) {
        eprintln!("Failed to report a clicked notification: {error}");
    }
}

/// Listens for clicks; must run before the first notification is shown.
pub fn init(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    macos::init(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// Resolves once the notification is shown, or with why it was not (macOS
/// reports a refusal; elsewhere it resolves once handed to the system).
#[tauri::command]
pub async fn show_notification(app: AppHandle, request: NotificationRequest) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return {
        let _ = app;
        macos::show(request).await
    };
    #[cfg(target_os = "windows")]
    return windows::show(&app, request);
    #[cfg(all(unix, not(target_os = "macos")))]
    return linux::show(&app, request);
}

#[tauri::command]
pub async fn notification_permission() -> NotificationPermission {
    #[cfg(target_os = "macos")]
    return macos::permission().await;
    #[cfg(not(target_os = "macos"))]
    NotificationPermission::Granted
}
