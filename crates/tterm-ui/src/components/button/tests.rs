use super::*;
use crate::tokens::palette::THEMES;
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

#[test]
fn style_reads_the_selected_theme_without_shared_state() {
    let before = style(
        Variant::Default,
        ButtonSize::Default,
        ButtonState::default(),
        THEMES[0],
    );
    for theme in THEMES {
        let selected = style(
            Variant::Default,
            ButtonSize::Default,
            ButtonState::default(),
            theme,
        );
        assert_eq!(selected.background, theme.colors.primary);
        assert_eq!(selected.foreground, theme.colors.primary_foreground);
    }
    assert_eq!(
        style(
            Variant::Default,
            ButtonSize::Default,
            ButtonState::default(),
            THEMES[0],
        ),
        before
    );
}

#[test]
fn hover_uses_the_variant_specific_background_and_border() {
    let light = style(
        Variant::Outline,
        ButtonSize::Default,
        ButtonState {
            hovered: true,
            ..ButtonState::default()
        },
        crate::tokens::palette::LIGHT,
    );
    assert_eq!(
        light.background,
        crate::tokens::palette::LIGHT.colors.accent
    );
    assert_eq!(light.border, crate::tokens::palette::LIGHT.colors.border);

    let dark = style(
        Variant::Outline,
        ButtonSize::Default,
        ButtonState {
            hovered: true,
            ..ButtonState::default()
        },
        crate::tokens::palette::DARK,
    );
    assert_eq!(dark.background, crate::tokens::palette::DARK.colors.input);
    assert_eq!(dark.background_alpha, 0.5);
    assert_eq!(dark.border, crate::tokens::palette::DARK.colors.input);
}

#[test]
fn disabled_keeps_each_variant_colour() {
    let destructive = style(
        Variant::Destructive,
        ButtonSize::Default,
        ButtonState {
            disabled: true,
            ..ButtonState::default()
        },
        crate::tokens::palette::LIGHT,
    );
    let outline = style(
        Variant::Outline,
        ButtonSize::Default,
        ButtonState {
            disabled: true,
            ..ButtonState::default()
        },
        crate::tokens::palette::LIGHT,
    );
    assert_eq!(
        destructive.background,
        crate::tokens::palette::LIGHT.colors.destructive
    );
    assert_eq!(destructive.foreground, WHITE);
    assert_eq!(
        outline.background,
        crate::tokens::palette::LIGHT.colors.background
    );
    assert_eq!(destructive.opacity, 0.5);
}

#[test]
fn react_variant_theme_hover_matrix() {
    use crate::tokens::palette::{DARK, LIGHT};
    for theme in [LIGHT, DARK] {
        let c = theme.colors;
        // Each tuple fixes non-hover/hover alpha, background, foreground,
        // focused ring colour/alpha, and underline from button.tsx classes.
        let rows = if theme.uses_dark_variant() {
            [
                // bg-primary text-primary-foreground hover:bg-primary/90; focus-visible:ring-ring/50.
                (
                    Variant::Default,
                    [1.0, 0.9],
                    [c.primary; 2],
                    [c.primary_foreground; 2],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // dark:bg-destructive/60 text-white hover:bg-destructive/90; dark:focus-visible:ring-destructive/40.
                (
                    Variant::Destructive,
                    [0.6, 0.9],
                    [c.destructive; 2],
                    [WHITE; 2],
                    c.destructive,
                    0.4,
                    [false; 2],
                ),
                // dark:bg-input/30 dark:hover:bg-input/50 hover:text-accent-foreground; ring-ring/50.
                (
                    Variant::Outline,
                    [0.3, 0.5],
                    [c.input; 2],
                    [c.foreground, c.accent_foreground],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // bg-secondary text-secondary-foreground hover:bg-secondary/80; ring-ring/50.
                (
                    Variant::Secondary,
                    [1.0, 0.8],
                    [c.secondary; 2],
                    [c.secondary_foreground; 2],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // transparent dark:hover:bg-accent/50 hover:text-accent-foreground; ring-ring/50.
                (
                    Variant::Ghost,
                    [0.0, 0.5],
                    [c.background, c.accent],
                    [c.foreground, c.accent_foreground],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // text-primary hover:underline, transparent; ring-ring/50.
                (
                    Variant::Link,
                    [0.0; 2],
                    [c.background; 2],
                    [c.primary; 2],
                    c.ring,
                    0.5,
                    [false, true],
                ),
            ]
        } else {
            [
                // bg-primary text-primary-foreground hover:bg-primary/90; focus-visible:ring-ring/50.
                (
                    Variant::Default,
                    [1.0, 0.9],
                    [c.primary; 2],
                    [c.primary_foreground; 2],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // bg-destructive text-white hover:bg-destructive/90; focus-visible:ring-destructive/20.
                (
                    Variant::Destructive,
                    [1.0, 0.9],
                    [c.destructive; 2],
                    [WHITE; 2],
                    c.destructive,
                    0.2,
                    [false; 2],
                ),
                // bg-background hover:bg-accent hover:text-accent-foreground; ring-ring/50.
                (
                    Variant::Outline,
                    [1.0; 2],
                    [c.background, c.accent],
                    [c.foreground, c.accent_foreground],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // bg-secondary text-secondary-foreground hover:bg-secondary/80; ring-ring/50.
                (
                    Variant::Secondary,
                    [1.0, 0.8],
                    [c.secondary; 2],
                    [c.secondary_foreground; 2],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // transparent hover:bg-accent hover:text-accent-foreground; ring-ring/50.
                (
                    Variant::Ghost,
                    [0.0, 1.0],
                    [c.background, c.accent],
                    [c.foreground, c.accent_foreground],
                    c.ring,
                    0.5,
                    [false; 2],
                ),
                // text-primary hover:underline, transparent; ring-ring/50.
                (
                    Variant::Link,
                    [0.0; 2],
                    [c.background; 2],
                    [c.primary; 2],
                    c.ring,
                    0.5,
                    [false, true],
                ),
            ]
        };
        for (variant, alpha, background, foreground, ring, ring_alpha, underline) in rows {
            for (index, hovered) in [false, true].into_iter().enumerate() {
                for focused in [false, true] {
                    let computed = style(
                        variant,
                        ButtonSize::Default,
                        ButtonState {
                            hovered,
                            focused,
                            disabled: false,
                        },
                        theme,
                    );
                    assert_eq!(
                        computed.background_alpha, alpha[index],
                        "{variant:?} {hovered} {}",
                        theme.id
                    );
                    assert_eq!(computed.background, background[index]);
                    assert_eq!(computed.foreground, foreground[index]);
                    assert_eq!(computed.ring, focused.then_some(ring));
                    assert_eq!(computed.ring_alpha, ring_alpha);
                    assert_eq!(computed.ring_spread, if focused { 3.0 } else { 0.0 });
                    assert_eq!(computed.underline, underline[index]);
                }
            }
        }
    }
}
