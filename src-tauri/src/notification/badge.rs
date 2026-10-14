//! How many system notifications no one has looked at yet, shown on the app
//! icon. macOS and Linux (launchers that read Unity's count) take the number
//! itself. Windows has no count for an app installed without a package
//! identity, so the frontend draws it and it goes on the taskbar button as an
//! overlay, the way chat apps show theirs. The tray icon gains a red dot as
//! well, since a window closed to the tray has no taskbar button to carry it.

use serde::Deserialize;
use tauri::AppHandle;

/// An overlay drawn by the frontend, as RGBA rows from the top.
#[derive(Debug, Clone, Deserialize)]
pub struct OverlayImage {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl OverlayImage {
    fn check(self) -> Result<Self, String> {
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

/// Shows `count` on the app icon, or takes it off at zero. `overlay` is the
/// count drawn for the Windows taskbar; other systems ignore it.
#[tauri::command]
pub async fn set_unseen_notifications(
    app: AppHandle,
    count: u32,
    overlay: Option<OverlayImage>,
) -> Result<(), String> {
    let overlay = overlay.map(OverlayImage::check).transpose()?;
    crate::background::tray::set_attention(&app, count > 0);
    #[cfg(target_os = "windows")]
    return windows_overlay::set(&app, overlay.filter(|_| count > 0));
    #[cfg(not(target_os = "windows"))]
    {
        let _ = overlay;
        set_badge_count(&app, count)
    }
}

#[cfg(not(target_os = "windows"))]
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
pub fn restore(app: &AppHandle) {
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

    use super::OverlayImage;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_must_hold_every_pixel() {
        let image = |len, width, height| OverlayImage {
            rgba: vec![0; len],
            width,
            height,
        };
        assert!(image(16 * 16 * 4, 16, 16).check().is_ok());
        assert!(image(16 * 16 * 4 - 1, 16, 16).check().is_err());
        assert!(image(0, 0, 0).check().is_err());
    }
}
