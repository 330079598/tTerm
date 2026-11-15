//! The first frame's colours, taken from the last saved theme background.
//!
//! This is the P3-01 bootstrap, not the P3-05 theme system: it reads the one
//! colour the Tauri client already saves in `app_meta`, picks the matching
//! built-in dark/light surface palette for the title bar, and paints the
//! content in the saved colour. Blur and material selection are not part of
//! this batch.

use gpui_kit::{Rgba, rgb};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

/// The dark background the application uses before a theme has ever saved
/// one. It matches `backgroundColor` in the Tauri window configuration.
pub const FALLBACK_BACKGROUND: (u8, u8, u8) = (0x1b, 0x1d, 0x23);

/// The same light/dark threshold as `is_dark_background` in
/// `src-tauri/src/window_blur.rs`.
pub fn is_dark_background((red, green, blue): (u8, u8, u8)) -> bool {
    0.299 * f64::from(red) + 0.587 * f64::from(green) + 0.114 * f64::from(blue) <= 128.0
}

/// The built-in surface palette that goes with the saved background.
pub fn theme(background: Option<(u8, u8, u8)>) -> Theme {
    if is_dark_background(background.unwrap_or(FALLBACK_BACKGROUND)) {
        DARK
    } else {
        LIGHT
    }
}

/// The content colour painted behind the terminal and the title bar.
pub fn content(background: Option<(u8, u8, u8)>) -> Rgba {
    let (red, green, blue) = background.unwrap_or(FALLBACK_BACKGROUND);
    rgb((u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue))
}

#[cfg(test)]
mod tests {
    use super::{FALLBACK_BACKGROUND, content, is_dark_background, theme};

    #[test]
    fn the_saved_background_chooses_the_surface_palette() {
        assert_eq!(theme(Some((0x1b, 0x1d, 0x23))).id, "dark");
        assert_eq!(theme(Some((0xff, 0xff, 0xff))).id, "light");
        assert_eq!(theme(None).id, "dark");
    }

    #[test]
    fn the_threshold_matches_the_existing_window_blur_rule() {
        assert!(is_dark_background((0x1b, 0x1d, 0x23)));
        assert!(!is_dark_background((0xff, 0xff, 0xff)));
        assert!(is_dark_background((128, 128, 128)));
        assert!(!is_dark_background((129, 129, 129)));
    }

    #[test]
    fn content_uses_the_saved_colour_or_the_dark_fallback() {
        let saved = content(Some((0x12, 0x34, 0x56)));
        assert_eq!(
            (saved.r * 255.0).round() as u8,
            0x12,
            "red channel round-trips"
        );
        assert_eq!((saved.g * 255.0).round() as u8, 0x34);
        assert_eq!((saved.b * 255.0).round() as u8, 0x56);

        let fallback = content(None);
        let (red, green, blue) = FALLBACK_BACKGROUND;
        assert_eq!((fallback.r * 255.0).round() as u8, red);
        assert_eq!((fallback.g * 255.0).round() as u8, green);
        assert_eq!((fallback.b * 255.0).round() as u8, blue);
    }
}
