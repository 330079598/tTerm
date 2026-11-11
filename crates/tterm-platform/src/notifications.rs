//! System notification data has no IPC or serialization requirements.
use crate::PlatformFuture;
/// A system notification and the tab to activate when clicked.
#[derive(Clone, Debug)]
pub struct NotificationRequest {
    pub title: String,
    pub subtitle: Option<String>,
    pub body: String,
    pub tab_id: Option<String>,
    pub sound: bool,
}
/// Existing permission distinctions, including development fallback and badge settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationPermission {
    Granted,
    Denied,
    NotDetermined,
    BadgeOff,
    TaskbarBadgesOff,
    Fallback,
}
/// Notification delivery is asynchronous; activation is supplied by the client callback.
pub trait Notifications: Send + Sync {
    /// Registers activation handling at the client's original startup position.
    fn initialize(&self) {}
    /// Hands a notification to the OS and reports delivery rejection.
    fn show(&self, request: NotificationRequest) -> PlatformFuture<'_, ()>;
    /// Queries notification and badge permission without granting it.
    fn permission(&self) -> PlatformFuture<'_, NotificationPermission>;
}
/// An RGBA overlay; its dimensions are validated before an OS API receives it.
#[derive(Clone, Debug)]
pub struct RgbaImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
impl RgbaImage {
    /// Rejects incomplete or zero-width images using the existing command's rule.
    pub fn check(self) -> Result<Self, String> {
        let expected = u64::from(self.width) * u64::from(self.height) * 4;
        if self.width == 0 || self.rgba.len() as u64 != expected {
            return Err(format!(
                "The notification count image is {} bytes, not {expected} for {}x{}",
                self.rgba.len(),
                self.width,
                self.height
            ));
        }
        Ok(self)
    }
}
/// Dock/taskbar badges are distinct from the tray attention dot.
pub trait AppBadge: Send + Sync {
    /// Updates the app icon count using an optional validated Windows overlay.
    fn set_unseen(&self, count: u32, overlay: Option<RgbaImage>) -> PlatformFuture<'_, ()>;
    /// Restores state after the taskbar button has been recreated.
    fn restore(&self) -> Result<(), String>;
}
