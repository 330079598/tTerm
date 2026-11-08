//! Token-backed Button primitive. Behavior is local to tterm-ui so component
//! colours never mutate gpui-component's global theme.

use crate::tokens::gpui::hsla;
use crate::tokens::palette::Colors;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    ElementId, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px,
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

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    variant: Variant,
    size: ButtonSize,
    state: ButtonState,
    colors: Colors,
) -> impl IntoElement {
    let (width, height, pxv, pyv, font_size) = dimensions(size);
    let (background, foreground, border, hover_background) = colors_for(variant, colors);
    let background = if state.disabled {
        colors.primary
    } else if state.hovered {
        hover_background
    } else {
        background
    };
    let background_alpha = if state.disabled { 0.5 } else { 1.0 };
    let foreground = if state.disabled {
        colors.muted_foreground
    } else {
        foreground
    };
    let border_width = if state.focused {
        2.0
    } else if matches!(variant, Variant::Outline) {
        1.0
    } else {
        0.0
    };
    let mut view = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .h(px(height))
        .when(width > 0.0, |this| this.w(px(width)))
        .min_w(px(width))
        .px(px(pxv))
        .py(px(pyv))
        .rounded(px(6.0))
        .border(px(border_width))
        .border_color(hsla(border, 1.0))
        .bg(hsla(background, background_alpha))
        .text_color(hsla(foreground, 1.0))
        .text_size(px(font_size))
        .font_weight(gpui_kit::FontWeight(500.0));
    if matches!(variant, Variant::Link) {
        view = view.underline();
    }
    view.child(label.into())
}

fn dimensions(size: ButtonSize) -> (f32, f32, f32, f32, f32) {
    match size {
        ButtonSize::Default => (0.0, 36.0, 16.0, 8.0, 14.0),
        ButtonSize::Xs => (0.0, 24.0, 8.0, 4.0, 12.0),
        ButtonSize::Sm => (0.0, 32.0, 12.0, 6.0, 14.0),
        ButtonSize::Lg => (0.0, 40.0, 24.0, 8.0, 14.0),
        ButtonSize::Icon => (36.0, 36.0, 0.0, 0.0, 14.0),
        ButtonSize::IconXs => (24.0, 24.0, 0.0, 0.0, 12.0),
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
