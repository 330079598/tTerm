//! Token-backed Button primitive. Behavior is local to tterm-ui so component
//! colours never mutate gpui-component's global theme.

use crate::tokens::gpui::hsla;
use crate::tokens::palette::Colors;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    BoxShadow, ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    div, px,
};

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
    pub line_height: f32,
    pub background: crate::tokens::color::Hsl,
    pub background_alpha: f32,
    pub foreground: crate::tokens::color::Hsl,
    pub border: crate::tokens::color::Hsl,
    pub border_width: f32,
    pub opacity: f32,
    pub ring: Option<crate::tokens::color::Hsl>,
}

pub fn style(
    variant: Variant,
    size: ButtonSize,
    state: ButtonState,
    colors: Colors,
) -> ButtonStyle {
    let (width, height, padding_x, padding_y, font_size) = dimensions(size);
    let (mut background, foreground, border, hover_background) = colors_for(variant, colors);
    if state.hovered {
        background = hover_background;
    }
    let dark = colors.background.lightness < 50.0;
    ButtonStyle {
        width,
        height,
        padding_x,
        padding_y,
        font_size,
        line_height: if matches!(size, ButtonSize::Xs | ButtonSize::IconXs) {
            13.8
        } else {
            16.8
        },
        background: if state.disabled {
            colors.primary
        } else {
            background
        },
        background_alpha: if state.disabled {
            1.0
        } else if matches!(variant, Variant::Destructive) && dark {
            0.6
        } else if matches!(variant, Variant::Outline) && dark {
            0.3
        } else {
            1.0
        },
        foreground: if state.disabled {
            colors.primary_foreground
        } else {
            foreground
        },
        border,
        border_width: if matches!(variant, Variant::Outline) && !state.focused {
            1.0
        } else {
            0.0
        },
        opacity: if state.disabled { 0.5 } else { 1.0 },
        ring: state.focused.then_some(colors.ring),
    }
}

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    size: ButtonSize,
    state: ButtonState,
    colors: Colors,
) -> impl IntoElement {
    let computed = style(variant, size, state, colors);
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
        .rounded(px(6.0))
        .border(px(computed.border_width))
        .border_color(hsla(computed.border, 1.0))
        .bg(crate::tokens::gpui::hsla(
            computed.background,
            computed.background_alpha,
        ))
        .text_color(hsla(computed.foreground, 1.0))
        .text_size(px(computed.font_size))
        .font_weight(gpui_kit::FontWeight(500.0))
        .opacity(computed.opacity);
    if let Some(ring) = computed.ring {
        view = view.shadow(vec![
            BoxShadow::new(px(0.0), px(0.0), hsla(ring, 0.5)).spread_radius(px(3.0)),
        ]);
    }
    if matches!(variant, Variant::Link) {
        view = view.underline();
    }
    view.child(label.into())
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
            colors.destructive_foreground,
            colors.destructive,
            colors.destructive,
        ),
        Variant::Outline => (
            colors.background,
            colors.foreground,
            colors.input,
            colors.accent,
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
mod tests {
    use super::*;
    #[test]
    fn exposes_every_react_variant_and_size() {
        assert_eq!(
            6,
            [
                Variant::Default,
                Variant::Destructive,
                Variant::Outline,
                Variant::Secondary,
                Variant::Ghost,
                Variant::Link
            ]
            .len()
        );
        assert_eq!(
            8,
            [
                ButtonSize::Default,
                ButtonSize::Xs,
                ButtonSize::Sm,
                ButtonSize::Lg,
                ButtonSize::Icon,
                ButtonSize::IconXs,
                ButtonSize::IconSm,
                ButtonSize::IconLg
            ]
            .len()
        );
    }
}
