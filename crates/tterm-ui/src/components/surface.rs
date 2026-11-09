//! CSS colour quantization and non-overlapping background/border painting.

use crate::tokens::{color::Hsl, gpui::hsla};
use gpui_kit::{IntoElement, Rgba, Styled, canvas, px, quad};

/// CSS rasterizes translucent colours through an 8-bit premultiplied surface.
/// Quantize that surface before GPUI blends it, preserving a transparent fill.
pub fn css_color(color: Hsl, alpha: f32) -> gpui_kit::Hsla {
    let rgba = Rgba::from(hsla(color, 1.0));
    let alpha_byte = (alpha.clamp(0.0, 1.0) * 255.0).round();
    let channel = |value: f32| {
        if alpha_byte == 0.0 {
            0.0
        } else if alpha_byte == 255.0 {
            (value * 255.0).round() / 255.0
        } else {
            (value * alpha_byte).floor() / alpha_byte
        }
    };
    Rgba {
        r: channel(rgba.r),
        g: channel(rgba.g),
        b: channel(rgba.b),
        a: alpha_byte / 255.0,
    }
    .into()
}

/// CSS opacity uses an 8-bit group opacity; GPUI otherwise multiplies floats.
pub fn css_opacity(opacity: f32) -> f32 {
    (opacity.clamp(0.0, 1.0) * 255.0).floor() / 255.0
}

/// Paints the fill inside the border so GPUI's per-primitive opacity cannot
/// blend a disabled border over an already translucent fill a second time.
pub fn surface(
    background: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    width: f32,
    ring: Option<gpui_kit::Hsla>,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            // Absolute children start at the parent's padding box.
            let bounds = bounds.inset(px(-width));
            if let Some(ring) = ring {
                window.paint_quad(quad(
                    bounds.inset(px(-3.0)),
                    px(9.0),
                    ring,
                    px(0.0),
                    gpui_kit::transparent_black(),
                    Default::default(),
                ));
            }
            window.paint_quad(quad(
                bounds.inset(px(width)),
                px((6.0 - width).max(0.0)),
                background,
                px(0.0),
                gpui_kit::transparent_black(),
                Default::default(),
            ));
            if width > 0.0 {
                window.paint_quad(quad(
                    bounds,
                    px(6.0),
                    gpui_kit::transparent_black(),
                    px(width),
                    border,
                    Default::default(),
                ));
            }
        },
    )
    .absolute()
    .inset_0()
}
