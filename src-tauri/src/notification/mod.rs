//! System notifications for terminal events: a long command finishing, a
//! notification a program asked for, a bell. Clicking one brings the window
//! forward on the tab that raised it.

pub mod badge;
pub(crate) mod sound;

use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationPermission {
    Granted,
    Denied,
    /// macOS asks the first time one is shown.
    NotDetermined,
    /// Allowed, but the app icon's count is turned off. macOS asks for badges
    /// only once, so a copy allowed before tTerm asked for them stays so until
    /// the user turns them on in System Settings.
    BadgeOff,
    /// Windows hides the count on taskbar buttons while "Show badges on
    /// taskbar apps" is off.
    TaskbarBadgesOff,
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
    use tauri::Manager;
    let _ = app
        .state::<std::sync::Arc<dyn tterm_platform::window::MainWindow>>()
        .show_and_focus();
    if let Err(error) = app.emit(ACTIVATED_EVENT, ActivatedPayload { tab_id }) {
        eprintln!("Failed to report a clicked notification: {error}");
    }
}

/// Listens for clicks; must run before the first notification is shown.
pub fn init(app: &AppHandle) {
    let native = tterm_platform::native::notification::NativeNotifications {
        identifier: crate::app_identity::get(app)
            .map(|id| id.identifier.clone())
            .unwrap_or_default(),
        activated: std::sync::Arc::new({
            let app = app.clone();
            move |tab_id| activated(&app, tab_id)
        }),
    };
    native.initialize();
    use tauri::Manager;
    app.manage(std::sync::Arc::new(native)
        as std::sync::Arc<dyn tterm_platform::notifications::Notifications>);
}

/// Resolves once the notification is shown, or with why it was not (macOS
/// reports a refusal; elsewhere it resolves once handed to the system).
#[tauri::command]
pub async fn show_notification(app: AppHandle, request: NotificationRequest) -> Result<(), String> {
    use tauri::Manager;
    app.state::<std::sync::Arc<dyn tterm_platform::notifications::Notifications>>()
        .show(tterm_platform::notifications::NotificationRequest {
            title: request.title,
            subtitle: request.subtitle,
            body: request.body,
            tab_id: request.tab_id,
            sound: request.sound,
        })
        .await
}

#[tauri::command]
pub async fn notification_permission(app: AppHandle) -> NotificationPermission {
    use tauri::Manager;
    use tterm_platform::notifications::NotificationPermission as P;
    let permission = app
        .state::<std::sync::Arc<dyn tterm_platform::notifications::Notifications>>()
        .permission()
        .await
        .unwrap_or(P::NotDetermined);
    match permission {
        P::Granted => NotificationPermission::Granted,
        P::Denied => NotificationPermission::Denied,
        P::NotDetermined => NotificationPermission::NotDetermined,
        P::BadgeOff => NotificationPermission::BadgeOff,
        P::TaskbarBadgesOff => NotificationPermission::TaskbarBadgesOff,
        P::Fallback => NotificationPermission::Fallback,
    }
}

/// Copies the audio file at `path` in as the notification sound and returns
/// its name. tTerm plays the sound itself because a Windows toast plays only
/// the system's sounds for an app that is not packaged.
#[tauri::command]
pub async fn import_notification_sound(path: String) -> Result<String, String> {
    crate::core::blocking::run_blocking(move || sound::import(std::path::Path::new(&path))).await
}

/// The notification sound's bytes, which the frontend decodes and plays.
#[tauri::command]
pub async fn read_notification_sound() -> Result<Response, String> {
    crate::core::blocking::run_blocking(sound::read)
        .await
        .map(Response::new)
}
