//! The unseen-notification count and overlay size, from
//! `src/lib/notificationBadge.ts`.
//!
//! Drawing the picture needs a canvas and stays with the clients; the native
//! badge renderer (`notification/badge.rs`) takes the RGBA pixels.

/// One digit fits a 16px overlay; two would be too small to read.
pub fn badge_label(count: i64) -> String {
    if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    }
}

/// Windows shows the overlay at 16px scaled with the display.
///
/// `Math.round` and Rust's `f64::round` agree for the positive ratios a
/// display reports.
pub fn badge_overlay_size(device_pixel_ratio: f64) -> u32 {
    (16.0 * device_pixel_ratio).round().max(16.0) as u32
}

#[cfg(test)]
mod tests {
    use super::{badge_label, badge_overlay_size};

    #[test]
    fn shows_a_single_digit_and_caps_the_rest() {
        assert_eq!(badge_label(1), "1");
        assert_eq!(badge_label(9), "9");
        assert_eq!(badge_label(10), "9+");
        assert_eq!(badge_label(250), "9+");
        assert_eq!(badge_label(-1), "-1");
    }

    #[test]
    fn follows_the_display_scale_and_never_drops_below_16px() {
        assert_eq!(badge_overlay_size(1.0), 16);
        assert_eq!(badge_overlay_size(1.5), 24);
        assert_eq!(badge_overlay_size(2.0), 32);
        assert_eq!(badge_overlay_size(0.5), 16);
    }
}
