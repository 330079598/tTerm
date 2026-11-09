//! Conversion from stylesheet tokens to GPUI values.

use super::{color::Hsl, font_size::FontSize};
use gpui_kit::{Hsla, Pixels};

/// Converts a stylesheet HSL token to GPUI's normalized HSLA representation.
pub fn hsla(color: Hsl, alpha: f32) -> Hsla {
    Hsla {
        h: color.hue.rem_euclid(360.0) / 360.0,
        s: (color.saturation / 100.0).clamp(0.0, 1.0),
        l: (color.lightness / 100.0).clamp(0.0, 1.0),
        a: alpha.clamp(0.0, 1.0),
    }
}

/// Converts a rem token to a scaled pixel size using the given root size.
pub fn pixels(size: FontSize, root_pixels: f32, scale_percent: f32) -> Pixels {
    Pixels::from(size.rem * root_pixels * (scale_percent / 100.0))
}

/// Converts a type token to its scaled font size and line height in pixels.
pub fn type_metrics(size: FontSize, root_pixels: f32, scale_percent: f32) -> (Pixels, Pixels) {
    let font_size = pixels(size, root_pixels, scale_percent);
    let line_height = Pixels::from(f32::from(font_size) * size.line_height);
    (font_size, line_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::type_scale::{TEXT_SM, TEXT_XL};

    #[test]
    fn converts_hsl_and_clamps_alpha() {
        let color = hsla(Hsl::new(360.0, 120.0, -10.0), 1.5);
        assert_eq!(color.h, 0.0);
        assert_eq!(color.s, 1.0);
        assert_eq!(color.l, 0.0);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn scales_font_size_and_line_height() {
        let (size, line_height) = type_metrics(TEXT_SM, 16.0, 100.0);
        assert_eq!(f32::from(size), 14.0);
        assert!((f32::from(line_height) - 16.8).abs() < 0.0001);

        let (size, line_height) = type_metrics(TEXT_XL, 16.0, 125.0);
        assert_eq!(f32::from(size), 25.0);
        assert!((f32::from(line_height) - 31.25).abs() < 0.0001);
    }
}
