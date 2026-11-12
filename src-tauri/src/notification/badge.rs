//! How many system notifications no one has looked at yet, shown on the app
//! icon. macOS and the Linux docks that follow Unity's LauncherEntry API take
//! the number itself. Windows has no count for an app installed without a package
//! identity, so the frontend draws it and it goes on the taskbar button as an
//! overlay, the way chat apps show theirs. The tray icon gains a red dot as
//! well, since a window closed to the tray has no taskbar button to carry it.

use serde::Deserialize;

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
pub(super) async fn set_unseen(
    window: &dyn tterm_platform::window::MainWindow,
    badge: &dyn tterm_platform::notifications::AppBadge,
    count: u32,
    overlay: Option<OverlayImage>,
) -> Result<(), String> {
    let overlay = overlay.map(OverlayImage::check).transpose()?;
    let _ = window.set_attention(count > 0);
    let overlay = overlay.map(|image| tterm_platform::notifications::RgbaImage {
        rgba: image.rgba,
        width: image.width,
        height: image.height,
    });
    badge.set_unseen(count, overlay).await
}
/// Restores a taskbar overlay after the main window is shown again.
pub fn restore(badge: &dyn tterm_platform::notifications::AppBadge) {
    if let Err(error) = badge.restore() {
        eprintln!("{error}");
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
