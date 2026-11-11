//! Existing OS notification implementations with client-owned activation routing.
#[cfg(all(unix, not(target_os = "macos")))]
pub mod launcher_entry;
#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
use crate::{
    PlatformFuture,
    notifications::{NotificationPermission, NotificationRequest, Notifications},
};
use std::sync::Arc;

/// Keeps the activation callback separate from any UI runtime or event format.
#[derive(Clone)]
pub struct NativeNotifications {
    pub identifier: String,
    pub activated: Arc<dyn Fn(Option<String>) + Send + Sync>,
}
impl NativeNotifications {
    /// Installs the macOS delegate before notifications are sent.
    pub fn initialize(&self) {
        #[cfg(target_os = "macos")]
        macos::init(self);
    }
}
fn activated(app: &NativeNotifications, tab_id: Option<String>) {
    (app.activated)(tab_id);
}
impl Notifications for NativeNotifications {
    fn initialize(&self) {
        NativeNotifications::initialize(self);
    }
    fn show(&self, request: NotificationRequest) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            #[cfg(target_os = "macos")]
            return macos::show(request).await;
            #[cfg(target_os = "windows")]
            return windows::show(self, request);
            #[cfg(all(unix, not(target_os = "macos")))]
            return linux::show(self, request);
        })
    }
    fn permission(&self) -> PlatformFuture<'_, NotificationPermission> {
        Box::pin(async {
            #[cfg(target_os = "macos")]
            return Ok(macos::permission().await);
            #[cfg(target_os = "windows")]
            return Ok(if windows::taskbar_badges_on() {
                NotificationPermission::Granted
            } else {
                NotificationPermission::TaskbarBadgesOff
            });
            #[cfg(all(unix, not(target_os = "macos")))]
            return Ok(NotificationPermission::Granted);
        })
    }
}
