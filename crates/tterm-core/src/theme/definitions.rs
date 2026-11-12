//! The six built-in themes and lookup helpers.

use std::collections::HashMap;
use std::sync::LazyLock;

use tterm_terminal::theme::{OptionalText, PaletteText, ThemeColors};

use crate::theme::types::{CustomTheme, PRESET_THEME_IDS, PresetTheme, Theme};

/// The built-in themes, in display order.
pub static PRESET_THEMES: LazyLock<Vec<PresetTheme>> = LazyLock::new(|| {
    vec![
        PresetTheme {
            id: "default".into(),
            name: "Default".into(),
            description: OptionalText::Value("Tabby inspired dark theme".into()),
            colors: ThemeColors {
                background: "220 13% 12%".into(),
                foreground: "0 0% 80%".into(),
                card: "220 13% 18%".into(),
                card_foreground: "0 0% 80%".into(),
                popover: "220 13% 18%".into(),
                popover_foreground: "0 0% 80%".into(),
                primary: "207 100% 40%".into(),
                primary_foreground: "0 0% 100%".into(),
                secondary: "220 13% 18%".into(),
                secondary_foreground: "0 0% 80%".into(),
                muted: "220 13% 23%".into(),
                muted_foreground: "0 0% 59%".into(),
                accent: "207 100% 40%".into(),
                accent_foreground: "0 0% 100%".into(),
                destructive: "0 84% 60%".into(),
                destructive_foreground: "0 0% 100%".into(),
                success: "142 72% 42%".into(),
                success_foreground: "142 76% 78%".into(),
                warning: "38 92% 52%".into(),
                warning_foreground: "42 96% 78%".into(),
                border: "0 0% 24%".into(),
                input: "0 0% 24%".into(),
                ring: "207 100% 40%".into(),
                tab_background: "220 13% 12%".into(),
                tab_active: "220 13% 20%".into(),
                tab_active_border: "207 100% 48%".into(),
                tab_hover: "220 13% 24%".into(),
                titlebar: "220 13% 16%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(220 13% 12%)",
                "#d7dde8",
                "#79d8ff",
                "rgba(77, 182, 255, 0.22)",
                [
                    "#161b22", "#f16c75", "#7bd88f", "#e6c16a", "#6aaeff", "#c792ea", "#66d9ef",
                    "#c8d2dc", "#5c6773", "#ff8b94", "#9ee6b8", "#f2d38f", "#8fc7ff", "#ddb7ff",
                    "#8be9fd", "#f4f7fb",
                ],
            ),
            is_custom: false,
        },
        PresetTheme {
            id: "light".into(),
            name: "Light".into(),
            description: OptionalText::Value("Clean light theme".into()),
            colors: ThemeColors {
                background: "0 0% 100%".into(),
                foreground: "222.2 84% 4.9%".into(),
                card: "0 0% 100%".into(),
                card_foreground: "222.2 84% 4.9%".into(),
                popover: "0 0% 100%".into(),
                popover_foreground: "222.2 84% 4.9%".into(),
                primary: "207 100% 40%".into(),
                primary_foreground: "210 40% 98%".into(),
                secondary: "210 40% 96.1%".into(),
                secondary_foreground: "222.2 47.4% 11.2%".into(),
                muted: "210 40% 96.1%".into(),
                muted_foreground: "215.4 16.3% 46.9%".into(),
                accent: "210 40% 96.1%".into(),
                accent_foreground: "222.2 47.4% 11.2%".into(),
                destructive: "0 84.2% 60.2%".into(),
                destructive_foreground: "210 40% 98%".into(),
                success: "142 72% 42%".into(),
                success_foreground: "142 50% 28%".into(),
                warning: "38 92% 52%".into(),
                warning_foreground: "30 75% 26%".into(),
                border: "214.3 31.8% 91.4%".into(),
                input: "214.3 31.8% 91.4%".into(),
                ring: "207 100% 40%".into(),
                tab_background: "0 0% 100%".into(),
                tab_active: "210 40% 93%".into(),
                tab_active_border: "207 100% 45%".into(),
                tab_hover: "210 40% 90%".into(),
                titlebar: "210 40% 96%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(0 0% 100%)",
                "#24292f",
                "#0969da",
                "rgba(9, 105, 218, 0.16)",
                [
                    "#24292f", "#cf222e", "#1a7f37", "#9a6700", "#0969da", "#8250df", "#1b7c83",
                    "#d0d7de", "#57606a", "#ff5c70", "#2da44e", "#bf8700", "#218bff", "#a475f9",
                    "#3192aa", "#f6f8fa",
                ],
            ),
            is_custom: false,
        },
        PresetTheme {
            id: "ocean".into(),
            name: "Ocean".into(),
            description: OptionalText::Value("Deep blue ocean theme".into()),
            colors: ThemeColors {
                background: "200 30% 10%".into(),
                foreground: "180 20% 85%".into(),
                card: "200 30% 15%".into(),
                card_foreground: "180 20% 85%".into(),
                popover: "200 30% 15%".into(),
                popover_foreground: "180 20% 85%".into(),
                primary: "190 80% 45%".into(),
                primary_foreground: "0 0% 100%".into(),
                secondary: "200 30% 20%".into(),
                secondary_foreground: "180 20% 85%".into(),
                muted: "200 30% 25%".into(),
                muted_foreground: "180 15% 60%".into(),
                accent: "170 70% 50%".into(),
                accent_foreground: "0 0% 100%".into(),
                destructive: "0 84% 60%".into(),
                destructive_foreground: "0 0% 100%".into(),
                success: "160 70% 44%".into(),
                success_foreground: "170 70% 78%".into(),
                warning: "38 92% 54%".into(),
                warning_foreground: "44 100% 80%".into(),
                border: "200 20% 30%".into(),
                input: "200 20% 30%".into(),
                ring: "190 80% 45%".into(),
                tab_background: "200 30% 10%".into(),
                tab_active: "200 30% 17%".into(),
                tab_active_border: "190 80% 48%".into(),
                tab_hover: "200 30% 22%".into(),
                titlebar: "200 30% 13%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(200 30% 10%)",
                "#d6f3f8",
                "#7de2ff",
                "rgba(50, 182, 214, 0.2)",
                [
                    "#102733", "#ff7a90", "#3fd0b3", "#e6c07b", "#4db5ff", "#b08df8", "#27d2e8",
                    "#cfeaf1", "#50707d", "#ff9eb2", "#79e6d0", "#f3d79f", "#86d0ff", "#d0b2ff",
                    "#76ebf5", "#f2fdff",
                ],
            ),
            is_custom: false,
        },
        PresetTheme {
            id: "forest".into(),
            name: "Forest".into(),
            description: OptionalText::Value("Natural green forest theme".into()),
            colors: ThemeColors {
                background: "140 25% 12%".into(),
                foreground: "120 15% 85%".into(),
                card: "140 25% 17%".into(),
                card_foreground: "120 15% 85%".into(),
                popover: "140 25% 17%".into(),
                popover_foreground: "120 15% 85%".into(),
                primary: "145 65% 45%".into(),
                primary_foreground: "0 0% 100%".into(),
                secondary: "140 25% 22%".into(),
                secondary_foreground: "120 15% 85%".into(),
                muted: "140 25% 27%".into(),
                muted_foreground: "120 10% 60%".into(),
                accent: "160 60% 50%".into(),
                accent_foreground: "0 0% 100%".into(),
                destructive: "0 84% 60%".into(),
                destructive_foreground: "0 0% 100%".into(),
                success: "145 65% 45%".into(),
                success_foreground: "125 48% 32%".into(),
                warning: "42 88% 56%".into(),
                warning_foreground: "42 88% 28%".into(),
                border: "140 15% 30%".into(),
                input: "140 15% 30%".into(),
                ring: "145 65% 45%".into(),
                tab_background: "140 25% 12%".into(),
                tab_active: "140 25% 19%".into(),
                tab_active_border: "145 65% 48%".into(),
                tab_hover: "140 25% 24%".into(),
                titlebar: "140 25% 15%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(140 25% 12%)",
                "#dbeed7",
                "#9adf8f",
                "rgba(104, 211, 145, 0.2)",
                [
                    "#172119", "#f07c82", "#69c26f", "#d5ba73", "#7fb8ff", "#c8a7ff", "#56c7ad",
                    "#cfddca", "#536458", "#ffb1aa", "#97dc8c", "#e7d59d", "#afd9ff", "#e4c6ff",
                    "#88e2cf", "#f4fff1",
                ],
            ),
            is_custom: false,
        },
        PresetTheme {
            id: "sunset".into(),
            name: "Sunset".into(),
            description: OptionalText::Value("Warm orange sunset theme".into()),
            colors: ThemeColors {
                background: "20 30% 12%".into(),
                foreground: "30 20% 85%".into(),
                card: "20 30% 17%".into(),
                card_foreground: "30 20% 85%".into(),
                popover: "20 30% 17%".into(),
                popover_foreground: "30 20% 85%".into(),
                primary: "25 85% 55%".into(),
                primary_foreground: "0 0% 100%".into(),
                secondary: "20 30% 22%".into(),
                secondary_foreground: "30 20% 85%".into(),
                muted: "20 30% 27%".into(),
                muted_foreground: "30 15% 60%".into(),
                accent: "340 75% 60%".into(),
                accent_foreground: "0 0% 100%".into(),
                destructive: "0 84% 60%".into(),
                destructive_foreground: "0 0% 100%".into(),
                success: "142 68% 44%".into(),
                success_foreground: "134 70% 82%".into(),
                warning: "32 94% 56%".into(),
                warning_foreground: "32 80% 24%".into(),
                border: "20 20% 30%".into(),
                input: "20 20% 30%".into(),
                ring: "25 85% 55%".into(),
                tab_background: "20 30% 12%".into(),
                tab_active: "20 30% 19%".into(),
                tab_active_border: "25 85% 55%".into(),
                tab_hover: "20 30% 24%".into(),
                titlebar: "20 30% 15%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(20 30% 12%)",
                "#f7e2d2",
                "#ff9e64",
                "rgba(255, 140, 92, 0.22)",
                [
                    "#2a1d1a", "#ff757f", "#9ccf6f", "#f6c177", "#7dcfff", "#f4a6ff", "#58d1c9",
                    "#dec8ba", "#6c5146", "#ff9aa4", "#c4e88d", "#ffdb9b", "#b6e3ff", "#ffc1ff",
                    "#8be9e2", "#fff5eb",
                ],
            ),
            is_custom: false,
        },
        PresetTheme {
            id: "ubuntu".into(),
            name: "Ubuntu".into(),
            description: OptionalText::Value("Ubuntu inspired theme".into()),
            colors: ThemeColors {
                background: "302 58% 10%".into(),
                foreground: "25 24% 92%".into(),
                card: "312 40% 17%".into(),
                card_foreground: "25 24% 92%".into(),
                popover: "312 40% 17%".into(),
                popover_foreground: "25 24% 92%".into(),
                primary: "14 82% 52%".into(),
                primary_foreground: "0 0% 100%".into(),
                secondary: "312 34% 20%".into(),
                secondary_foreground: "25 24% 92%".into(),
                muted: "312 30% 23%".into(),
                muted_foreground: "25 12% 72%".into(),
                accent: "312 34% 22%".into(),
                accent_foreground: "25 24% 92%".into(),
                destructive: "0 72% 51%".into(),
                destructive_foreground: "0 0% 100%".into(),
                success: "120 52% 46%".into(),
                success_foreground: "120 52% 86%".into(),
                warning: "38 88% 52%".into(),
                warning_foreground: "38 80% 32%".into(),
                border: "312 22% 27%".into(),
                input: "312 22% 27%".into(),
                ring: "14 82% 52%".into(),
                tab_background: "302 58% 10%".into(),
                tab_active: "312 36% 18%".into(),
                tab_active_border: "14 82% 52%".into(),
                tab_hover: "312 34% 23%".into(),
                titlebar: "302 50% 13%".into(),
                extra: Default::default(),
            },
            terminal: palette(
                "hsl(302 58% 10%)",
                "#eeeeec",
                "#e95420",
                "rgba(233, 84, 32, 0.26)",
                [
                    "#2e3436", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a",
                    "#d3d7cf", "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8",
                    "#34e2e2", "#ffffff",
                ],
            ),
            is_custom: false,
        },
    ]
});

/// The built-in themes keyed by id.
pub static PRESET_THEME_MAP: LazyLock<HashMap<&'static str, &'static PresetTheme>> =
    LazyLock::new(|| {
        PRESET_THEMES
            .iter()
            .map(|theme| (theme.id.as_str(), theme))
            .collect()
    });

fn palette(
    background: &str,
    foreground: &str,
    cursor: &str,
    selection_background: &str,
    ansi: [&str; 16],
) -> PaletteText {
    PaletteText {
        background: background.into(),
        foreground: foreground.into(),
        cursor: cursor.into(),
        cursor_accent: OptionalText::Missing,
        selection_background: selection_background.into(),
        black: ansi[0].into(),
        red: ansi[1].into(),
        green: ansi[2].into(),
        yellow: ansi[3].into(),
        blue: ansi[4].into(),
        magenta: ansi[5].into(),
        cyan: ansi[6].into(),
        white: ansi[7].into(),
        bright_black: ansi[8].into(),
        bright_red: ansi[9].into(),
        bright_green: ansi[10].into(),
        bright_yellow: ansi[11].into(),
        bright_blue: ansi[12].into(),
        bright_magenta: ansi[13].into(),
        bright_cyan: ansi[14].into(),
        bright_white: ansi[15].into(),
        selection_foreground: OptionalText::Missing,
        extra: Default::default(),
    }
}

/// Returns a built-in theme by id.
pub fn get_preset_theme(theme_id: &str) -> Option<&'static PresetTheme> {
    if !PRESET_THEME_IDS.contains(&theme_id) {
        return None;
    }
    PRESET_THEME_MAP.get(theme_id).copied()
}

/// Resolves a theme id against custom themes, then the presets, then the
/// built-in default.
pub fn resolve_theme_definition(theme_id: &str, custom_themes: &[CustomTheme]) -> Theme {
    if let Some(theme) = custom_themes.iter().find(|theme| theme.id == theme_id) {
        return Theme::Custom(Box::new(theme.clone()));
    }
    if let Some(theme) = get_preset_theme(theme_id) {
        return Theme::Preset(theme.clone());
    }
    if let Some(theme) = get_preset_theme("default") {
        return Theme::Preset(theme.clone());
    }
    Theme::Preset(default_theme())
}

fn default_theme() -> PresetTheme {
    PresetTheme {
        id: "default".into(),
        name: "Default".into(),
        description: OptionalText::Value("Tabby inspired dark theme".into()),
        colors: ThemeColors::dark_default(),
        terminal: PaletteText {
            background: "hsl(220 13% 12%)".into(),
            foreground: "#d7dde8".into(),
            cursor: "#79d8ff".into(),
            selection_background: "rgba(77, 182, 255, 0.22)".into(),
            black: "#161b22".into(),
            red: "#f16c75".into(),
            green: "#7bd88f".into(),
            yellow: "#e6c16a".into(),
            blue: "#6aaeff".into(),
            magenta: "#c792ea".into(),
            cyan: "#66d9ef".into(),
            white: "#c8d2dc".into(),
            bright_black: "#5c6773".into(),
            bright_red: "#ff8b94".into(),
            bright_green: "#9ee6b8".into(),
            bright_yellow: "#f2d38f".into(),
            bright_blue: "#8fc7ff".into(),
            bright_magenta: "#ddb7ff".into(),
            bright_cyan: "#8be9fd".into(),
            bright_white: "#f4f7fb".into(),
            ..Default::default()
        },
        is_custom: false,
    }
}

#[cfg(test)]
mod tests;
