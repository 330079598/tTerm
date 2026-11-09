//! Token-backed Button primitive. Behavior is local to tterm-ui so component
//! colours never mutate gpui-component's global theme.

use crate::components::surface::{css_color, css_opacity, surface};
use crate::tokens::gpui::hsla;
use crate::tokens::palette::Colors;
use crate::tokens::theme::Theme;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{ElementId, InteractiveElement, ParentElement, SharedString, Styled, div, px};

/// React's `text-white` destructive variant is independent of theme tokens.
const WHITE: crate::tokens::color::Hsl = crate::tokens::color::Hsl::new(0.0, 0.0, 100.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Default,
    Xs,
    Sm,
    Lg,
    Icon,
    IconXs,
    IconSm,
    IconLg,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub hovered: bool,
    pub focused: bool,
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonStyle {
    pub width: f32,
    pub height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub font_size: f32,
    pub radius: f32,
    pub weight: u16,
    pub line_height: f32,
    pub background: crate::tokens::color::Hsl,
    pub background_alpha: f32,
    pub foreground: crate::tokens::color::Hsl,
    pub border: crate::tokens::color::Hsl,
    pub border_width: f32,
    pub opacity: f32,
    pub ring: Option<crate::tokens::color::Hsl>,
    /// Outer shadow spread in logical pixels, shared with parity snapshots.
    pub ring_spread: f32,
}

pub fn style(variant: Variant, size: ButtonSize, state: ButtonState, theme: Theme) -> ButtonStyle {
    let colors = theme.colors;
    let (width, height, padding_x, padding_y, font_size) = dimensions(size);
    let (mut background, mut foreground, border, hover_background) =
        colors_for(variant, colors, theme.uses_dark_variant());
    if state.hovered {
        background = hover_background;
        if matches!(variant, Variant::Outline | Variant::Ghost) {
            foreground = colors.accent_foreground;
        }
    }
    ButtonStyle {
        width,
        height,
        padding_x,
        padding_y,
        font_size,
        radius: 6.0,
        weight: 500,
        line_height: if matches!(size, ButtonSize::Xs) {
            13.8
        } else {
            16.8
        },
        background,
        background_alpha: if matches!(variant, Variant::Link)
            || (matches!(variant, Variant::Ghost) && !state.hovered)
        {
            0.0
        } else if matches!(variant, Variant::Ghost) && theme.uses_dark_variant() {
            0.5
        } else if matches!(variant, Variant::Outline) && theme.uses_dark_variant() {
            if state.hovered { 0.5 } else { 0.3 }
        } else if matches!(variant, Variant::Destructive) && theme.uses_dark_variant() {
            0.6
        } else if state.hovered && matches!(variant, Variant::Default | Variant::Destructive) {
            0.9
        } else if state.hovered && matches!(variant, Variant::Secondary) {
            0.8
        } else {
            1.0
        },
        foreground,
        border,
        border_width: if matches!(variant, Variant::Outline) && !state.focused {
            1.0
        } else {
            0.0
        },
        opacity: if state.disabled { 0.5 } else { 1.0 },
        ring: state.focused.then_some(colors.ring),
        ring_spread: if state.focused { 3.0 } else { 0.0 },
    }
}

/// Builds a button. The result is a stateful `Div`, so callers attach
/// `on_click` and icon children; an empty label adds no text node, which keeps
/// icon-only buttons centred the way React's `size="icon*"` buttons are.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    size: ButtonSize,
    state: ButtonState,
    theme: Theme,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let computed = style(variant, size, state, theme);
    let mut view = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .h(px(computed.height))
        .when(computed.width > 0.0, |this| this.w(px(computed.width)))
        .min_w(px(computed.width))
        .px(px(computed.padding_x))
        .py(px(computed.padding_y))
        .rounded(px(computed.radius))
        .border(px(computed.border_width))
        .border_color(gpui_kit::transparent_black())
        .relative()
        .text_color(hsla(computed.foreground, 1.0))
        .text_size(px(computed.font_size))
        .line_height(px(computed.line_height))
        .font_weight(gpui_kit::FontWeight(f32::from(computed.weight)))
        .opacity(css_opacity(computed.opacity))
        .child(surface(
            css_color(computed.background, computed.background_alpha),
            css_color(computed.border, 1.0),
            computed.border_width,
            computed.radius,
            computed
                .ring
                .map(|ring| (css_color(ring, 0.5), computed.ring_spread)),
        ));
    if matches!(variant, Variant::Link) {
        view = view.underline();
    }
    let label = label.into();
    if label.is_empty() {
        view
    } else {
        view.child(label)
    }
}

fn dimensions(size: ButtonSize) -> (f32, f32, f32, f32, f32) {
    match size {
        ButtonSize::Default => (0.0, 36.0, 16.0, 8.0, 14.0),
        ButtonSize::Xs => (0.0, 24.0, 8.0, 0.0, 12.0),
        ButtonSize::Sm => (0.0, 32.0, 12.0, 0.0, 14.0),
        ButtonSize::Lg => (0.0, 40.0, 24.0, 0.0, 14.0),
        ButtonSize::Icon => (36.0, 36.0, 0.0, 0.0, 14.0),
        ButtonSize::IconXs => (24.0, 24.0, 0.0, 0.0, 14.0),
        ButtonSize::IconSm => (32.0, 32.0, 0.0, 0.0, 14.0),
        ButtonSize::IconLg => (40.0, 40.0, 0.0, 0.0, 14.0),
    }
}

fn colors_for(
    variant: Variant,
    colors: Colors,
    dark_variant: bool,
) -> (
    crate::tokens::color::Hsl,
    crate::tokens::color::Hsl,
    crate::tokens::color::Hsl,
    crate::tokens::color::Hsl,
) {
    match variant {
        Variant::Default => (
            colors.primary,
            colors.primary_foreground,
            colors.primary,
            colors.primary,
        ),
        Variant::Destructive => (
            colors.destructive,
            WHITE,
            colors.destructive,
            colors.destructive,
        ),
        Variant::Outline => (
            if dark_variant {
                colors.input
            } else {
                colors.background
            },
            colors.foreground,
            if dark_variant {
                colors.input
            } else {
                colors.border
            },
            if dark_variant {
                colors.input
            } else {
                colors.accent
            },
        ),
        Variant::Secondary => (
            colors.secondary,
            colors.secondary_foreground,
            colors.secondary,
            colors.secondary,
        ),
        Variant::Ghost => (
            colors.background,
            colors.foreground,
            colors.background,
            colors.accent,
        ),
        Variant::Link => (
            colors.background,
            colors.primary,
            colors.background,
            colors.background,
        ),
    }
}

#[cfg(test)]
mod tests;
