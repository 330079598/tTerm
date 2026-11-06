//! Colour conversion shared by painting and the exported layout.
//!
//! The React stylesheet states colours as `hsl(h s% l%)`. CSS converts them to
//! 8-bit sRGB and the browser reports the rounded channels; the layout JSON
//! records those channels. This module converts the same token values the same
//! way, and the layout comparison then compares integers rather than two
//! floating-point colour spaces.

use gpui_kit::Rgba;
use tterm_ui::tokens::color::Hsl;

/// Converts an interface token to 8-bit sRGB, opaque.
pub fn hsl_to_rgba(color: Hsl) -> [u8; 4] {
    let hue = color.hue.rem_euclid(360.0) / 60.0;
    let saturation = color.saturation / 100.0;
    let lightness = color.lightness / 100.0;

    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let second = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let offset = lightness - chroma / 2.0;

    let (red, green, blue) = match hue as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };

    [
        channel(red + offset),
        channel(green + offset),
        channel(blue + offset),
        255,
    ]
}

/// Converts an 8-bit sRGB token to the colour GPUI paints.
pub fn to_gpui(color: [u8; 4]) -> Rgba {
    if color[3] == 255 {
        gpui_kit::rgb(u32::from(color[0]) << 16 | u32::from(color[1]) << 8 | u32::from(color[2]))
    } else {
        gpui_kit::rgba(
            u32::from(color[0]) << 24
                | u32::from(color[1]) << 16
                | u32::from(color[2]) << 8
                | u32::from(color[3]),
        )
    }
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_ui::tokens::palette::{DARK, LIGHT};

    #[test]
    fn dark_theme_tokens_match_the_stylesheet() {
        assert_eq!(hsl_to_rgba(DARK.colors.background), [27, 29, 35, 255]);
        assert_eq!(hsl_to_rgba(DARK.colors.foreground), [204, 204, 204, 255]);
        assert_eq!(hsl_to_rgba(DARK.colors.primary), [0, 112, 204, 255]);
        assert_eq!(
            hsl_to_rgba(DARK.colors.primary_foreground),
            [255, 255, 255, 255]
        );
        assert_eq!(hsl_to_rgba(DARK.colors.border), [61, 61, 61, 255]);
        assert_eq!(hsl_to_rgba(DARK.colors.input), [61, 61, 61, 255]);
        assert_eq!(
            hsl_to_rgba(DARK.colors.muted_foreground),
            [150, 150, 150, 255]
        );
    }

    #[test]
    fn light_theme_tokens_match_the_stylesheet() {
        assert_eq!(hsl_to_rgba(LIGHT.colors.background), [255, 255, 255, 255]);
        assert_eq!(hsl_to_rgba(LIGHT.colors.foreground), [2, 8, 23, 255]);
        assert_eq!(hsl_to_rgba(LIGHT.colors.primary), [0, 112, 204, 255]);
        assert_eq!(hsl_to_rgba(LIGHT.colors.border), [226, 232, 240, 255]);
    }

    #[test]
    fn hue_wraps_and_extremes_clamp() {
        assert_eq!(
            hsl_to_rgba(Hsl::new(360.0, 100.0, 50.0)),
            hsl_to_rgba(Hsl::new(0.0, 100.0, 50.0))
        );
        assert_eq!(hsl_to_rgba(Hsl::new(0.0, 0.0, 0.0)), [0, 0, 0, 255]);
        assert_eq!(hsl_to_rgba(Hsl::new(0.0, 0.0, 100.0)), [255, 255, 255, 255]);
    }

    #[test]
    fn gpui_colours_keep_their_channels() {
        let color = to_gpui([0, 112, 204, 255]);
        assert_eq!((color.r * 255.0).round() as u8, 0);
        assert_eq!((color.g * 255.0).round() as u8, 112);
        assert_eq!((color.b * 255.0).round() as u8, 204);
    }
}
