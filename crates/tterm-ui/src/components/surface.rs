//! CSS colour quantization and non-overlapping background/border painting.

use crate::tokens::{color::Hsl, gpui::hsla};
use gpui_kit::{BoxShadow, IntoElement, Rgba, Styled, canvas, px, quad};

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

/// Composites an opaque colour at `opacity` over an opaque `backdrop`, the way
/// CSS `opacity` flattens an element group before blending it.
///
/// GPUI applies element opacity to every primitive separately, so a child
/// painted over its parent's fill lets that fill show through it. Components
/// whose disabled state overlaps opaque layers paint these precomposited
/// colours at full opacity instead.
pub fn css_group_color(color: Hsl, opacity: f32, backdrop: Hsl) -> gpui_kit::Hsla {
    let front = Rgba::from(hsla(color, 1.0));
    let back = Rgba::from(hsla(backdrop, 1.0));
    let alpha = css_opacity(opacity);
    let channel = |front: f32, back: f32| {
        let front = (front * 255.0).round();
        let back = (back * 255.0).round();
        (front * alpha + back * (1.0 - alpha)).round() / 255.0
    };
    Rgba {
        r: channel(front.r, back.r),
        g: channel(front.g, back.g),
        b: channel(front.b, back.b),
        a: 1.0,
    }
    .into()
}

/// Flattens a CSS alpha fill onto its opaque backdrop before adding a shadow.
/// GPUI shadows also paint beneath the box, so an unflattened fill would let
/// shadow ink change the interior colour. Preserve CSS premultiplied rounding.
pub fn css_backdrop_color(color: Hsl, alpha: f32, backdrop: Hsl) -> gpui_kit::Hsla {
    let front = Rgba::from(css_color(color, alpha));
    let back = Rgba::from(css_color(backdrop, 1.0));
    let channel = |front_channel: f32, back_channel: f32| {
        ((front_channel * front.a + back_channel * (1.0 - front.a)) * 255.0).round() / 255.0
    };
    Rgba {
        r: channel(front.r, back.r),
        g: channel(front.g, back.g),
        b: channel(front.b, back.b),
        a: 1.0,
    }
    .into()
}

/// Tailwind v4's `shadow-xs`: `0 1px 2px 0 rgb(0 0 0 / 0.05)`.
///
/// GPUI's blur radius is the Gaussian sigma while CSS's is twice the sigma,
/// so the CSS 2px blur becomes 1px. `opacity` is the element's CSS opacity,
/// which applies to its shadow as part of the same group.
pub fn shadow_xs(opacity: f32) -> BoxShadow {
    let color = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.05 * css_opacity(opacity),
    };
    BoxShadow::new(px(0.0), px(1.0), color.into()).blur_radius(px(1.0))
}

/// Tailwind v4's `shadow-sm`: `0 1px 3px 0 rgb(0 0 0 / 0.1),
/// 0 1px 2px -1px rgb(0 0 0 / 0.1)`.
pub fn shadow_sm(opacity: f32) -> Vec<BoxShadow> {
    let alpha = 0.1 * css_opacity(opacity);
    vec![
        BoxShadow::new(px(0.0), px(1.0), gpui_kit::black().opacity(alpha)).blur_radius(px(1.5)),
        BoxShadow::new(px(0.0), px(1.0), gpui_kit::black().opacity(alpha))
            .blur_radius(px(1.0))
            .spread_radius(px(-1.0)),
    ]
}

/// CSS paints the offset above the outer ring. Both shadows stay hollow,
/// preserving a trigger's transparent centre and the parent's own backdrop.
pub fn offset_ring(
    background: gpui_kit::Hsla,
    ring: gpui_kit::Hsla,
    radius: f32,
    spread: f32,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            for (color, outset) in [(ring, spread), (background, 2.0)] {
                let mut clear = color;
                clear.a = 0.0;
                window.paint_quad(quad(
                    bounds.inset(px(-outset)),
                    px(spread_radius(radius, outset)),
                    clear,
                    px(outset),
                    color,
                    Default::default(),
                ));
            }
        },
    )
    .absolute()
    .inset_0()
}

/// The outer corner radius of a CSS spread shadow (CSS Backgrounds 3, 6.1.1):
/// corners grow by the spread, and small radii stay sharper.
pub fn spread_radius(radius: f32, spread: f32) -> f32 {
    if spread > 0.0 && radius < spread {
        radius + spread * (1.0 + (radius / spread - 1.0).powi(3))
    } else {
        radius + spread
    }
}

/// Paints the fill inside the border so GPUI's per-primitive opacity cannot
/// blend a disabled border over an already translucent fill a second time.
///
/// `width` must be the parent's border width: absolute children start at the
/// parent's padding box, and the border box is recovered from it. `radius` is
/// the parent's border-box corner radius.
pub fn surface(
    background: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    width: f32,
    radius: f32,
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
                    px(spread_radius(radius, spread)),
                    clear_ring,
                    px(spread),
                    ring,
                    Default::default(),
                ));
            }
            window.paint_quad(quad(
                bounds.inset(px(width)),
                px((radius - width).max(0.0)),
                background,
                px(0.0),
                gpui_kit::transparent_black(),
                Default::default(),
            ));
            if width > 0.0 {
                window.paint_quad(quad(
                    bounds,
                    px(radius),
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

    #[test]
    fn translucent_fill_is_flattened_before_shadow_paint() {
        let rgba = Rgba::from(css_backdrop_color(
            DARK.colors.input,
            0.3,
            DARK.colors.background,
        ));
        let bytes = [rgba.r, rgba.g, rgba.b, rgba.a].map(|channel| (channel * 255.0).round() as u8);
        assert_eq!(bytes, [37, 38, 42, 255]);
        assert_eq!(
            css_backdrop_color(DARK.colors.input, 0.0, DARK.colors.background),
            css_color(DARK.colors.background, 1.0)
        );
    }

    #[test]
    fn shadow_sm_converts_css_blur_to_sigma_and_group_opacity() {
        let shadows = shadow_sm(0.5);
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].blur_radius, px(1.5));
        assert_eq!(shadows[1].blur_radius, px(1.0));
        assert_eq!(shadows[1].spread_radius, px(-1.0));
        for shadow in shadows {
            assert_eq!(shadow.offset.y, px(1.0));
            assert!((shadow.color.a - 0.1 * css_opacity(0.5)).abs() < 0.00001);
        }
    }

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
