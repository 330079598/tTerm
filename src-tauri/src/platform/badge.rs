//! Only window API calls remain here; Linux's DBus implementation is shared.
use tauri::AppHandle;
use tterm_platform::{
    PlatformFuture,
    notifications::{AppBadge, RgbaImage},
};
pub(super) struct TauriBadge(pub AppHandle);
impl AppBadge for TauriBadge {
    fn set_unseen(&self, count: u32, overlay: Option<RgbaImage>) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            #[cfg(windows)]
            return windows_overlay::set(&self.0, overlay.filter(|_| count > 0));
            #[cfg(target_os = "macos")]
            {
                let _ = overlay;
                set_badge_count(&self.0, count)
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                let _ = overlay;
                tterm_platform::native::notification::launcher_entry::set_count(
                    &crate::app_identity::get(&self.0)?.name,
                    count,
                )
                .await
            }
        })
    }
    fn restore(&self) -> Result<(), String> {
        restore_overlay(&self.0);
        Ok(())
    }
}
#[cfg(target_os = "macos")]
fn set_badge_count(app: &AppHandle, count: u32) -> Result<(), String> {
    use tauri::Manager;

    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .set_badge_count((count > 0).then_some(i64::from(count)))
        .map_err(|error| format!("Failed to show the notification count: {error}"))
}

/// Puts the overlay back on a taskbar button made anew, as one is when the
/// window comes back from the tray.
fn restore_overlay(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    if let Err(error) = windows_overlay::restore(app) {
        eprintln!("{error}");
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

#[cfg(target_os = "windows")]
mod windows_overlay {
    use std::sync::Mutex;

    use tauri::image::Image;
    use tauri::{AppHandle, Manager};

    use tterm_platform::notifications::RgbaImage as OverlayImage;

    /// The overlay shown now, kept to put back on a new taskbar button.
    static SHOWN: Mutex<Option<OverlayImage>> = Mutex::new(None);

    pub(super) fn set(app: &AppHandle, overlay: Option<OverlayImage>) -> Result<(), String> {
        let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        *shown = overlay;
        apply(app, shown.as_ref())
    }

    pub(super) fn restore(app: &AppHandle) -> Result<(), String> {
        let shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        if shown.is_none() {
            return Ok(());
        }
        apply(app, shown.as_ref())
    }

    fn apply(app: &AppHandle, overlay: Option<&OverlayImage>) -> Result<(), String> {
        let Some(window) = app.get_webview_window("main") else {
            return Ok(());
        };
        let icon = overlay.map(|image| Image::new(&image.rgba, image.width, image.height));
        window
            .set_overlay_icon(icon)
            .map_err(|error| format!("Failed to show the notification count: {error}"))
    }
}
