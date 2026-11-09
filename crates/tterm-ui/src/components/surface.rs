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
    ring: Option<(gpui_kit::Hsla, f32)>,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            // Absolute children start at the parent's padding box.
            let bounds = bounds.inset(px(-width));
            if let Some((ring, spread)) = ring {
                // CSS outer shadows exclude the border box, including when
                // the button fill is transparent. Keep both AA edges separate.
                // GPUI interpolates straight RGB as well as alpha at the inner
                // edge; retain the border RGB in the zero-alpha fill to avoid
                // multiplying coverage into the colour a second time.
                let mut clear_ring = ring;
                clear_ring.a = 0.0;
                window.paint_quad(quad(
                    bounds.inset(px(-spread)),
                    px(6.0 + spread),
                    clear_ring,
                    px(spread),
                    ring,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::{ButtonSize, ButtonState, Variant, button, style};
    use crate::tokens::palette::DARK;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{ParentElement, Render, TestAppContext, div};

    struct FocusedButton(Variant);

    impl Render for FocusedButton {
        fn render(
            &mut self,
            _window: &mut gpui_kit::Window,
            _cx: &mut gpui_kit::Context<Self>,
        ) -> impl IntoElement {
            div().p(px(20.0)).child(button(
                "focus-surface",
                "",
                self.0,
                ButtonSize::Default,
                ButtonState {
                    focused: true,
                    ..ButtonState::default()
                },
                DARK,
            ))
        }
    }

    #[gpui_kit::test]
    fn outer_ring_stays_hollow_for_transparent_and_translucent_buttons(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for variant in [Variant::Ghost, Variant::Outline] {
            let computed = style(
                variant,
                ButtonSize::Default,
                ButtonState {
                    focused: true,
                    ..ButtonState::default()
                },
                DARK,
            );
            assert!(computed.background_alpha < 1.0);
            let (_, visual) = cx.add_window_view(|_, _| FocusedButton(variant));
            visual.update(|window, cx| {
                window.render_frame(cx);
                let ring_color = css_color(computed.ring.expect("focused ring"), 0.5);
                let quads = window.painted_quads();
                let ring = quads
                    .iter()
                    .find(|quad| quad.border_color == ring_color)
                    .expect("painted ring border");
                let scale = window.scale_factor();
                let mut clear_ring = ring_color;
                clear_ring.a = 0.0;
                assert_eq!(ring.background, clear_ring.into());
                assert_eq!(ring.border_widths.top.as_f32(), 3.0 * scale);
                assert_eq!(ring.border_widths.left, ring.border_widths.top);
                assert_eq!(ring.border_widths.right, ring.border_widths.top);
                assert_eq!(ring.border_widths.bottom, ring.border_widths.top);
                assert_eq!(ring.corner_radii.top_left.as_f32(), 9.0 * scale);
                // The ring hole is exactly the button border box: its centre
                // never contains ring paint, regardless of the fill's alpha.
                assert_eq!(ring.bounds.origin.x.as_f32(), 17.0 * scale);
                assert_eq!(ring.bounds.origin.y.as_f32(), 17.0 * scale);
                assert_eq!(
                    ring.bounds.size.height.as_f32(),
                    (computed.height + 6.0) * scale
                );
                assert_eq!(
                    ring.corner_radii.top_left.as_f32() - ring.border_widths.top.as_f32(),
                    6.0 * scale
                );
            });
        }
    }
}
