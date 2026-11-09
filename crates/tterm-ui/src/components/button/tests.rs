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
