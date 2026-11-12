//! Turning a terminal color scheme into a terminal palette and interface
//! colors, following `src/lib/themeDerivation.ts`.

use serde::{Deserialize, Serialize};

use tterm_terminal::theme::js::{js_number_to_string, js_round};
use tterm_terminal::theme::{
    OptionalText, PaletteText, ThemeColors, color_to_hex, contrast_ratio, hex_to_rgb,
    relative_luminance, rgb_to_hex,
};

const ANSI_KEYS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
];

const WHITE: &str = "#ffffff";
const BLACK: &str = "#000000";

/// A terminal color scheme as theme files and the library describe one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalScheme {
    /// The scheme name, when the file has one.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub name: OptionalText,
    /// The background color.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub background: OptionalText,
    /// The foreground color.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub foreground: OptionalText,
    /// ANSI colors 0-15; a missing entry keeps the background or foreground.
    pub palette: Vec<Option<String>>,
    /// The cursor color.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub cursor: OptionalText,
    /// Text drawn inside a block cursor.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub cursor_text: OptionalText,
    /// The selection background.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub selection_background: OptionalText,
    /// Selected text.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub selection_foreground: OptionalText,
}

/// Whether the background is darker than the foreground.
pub fn is_dark_palette(palette: &PaletteText) -> bool {
    let background = relative_luminance(&hex(&palette.background, BLACK)).unwrap_or(0.0);
    let foreground = relative_luminance(&hex(&palette.foreground, WHITE)).unwrap_or(1.0);
    background < foreground
}

/// Maps a scheme onto the terminal palette, filling the colors it leaves
/// out the way `terminalPaletteFromScheme` does.
pub fn terminal_palette_from_scheme(scheme: &TerminalScheme) -> PaletteText {
    let background = hex(scheme.background.as_str().unwrap_or_default(), BLACK);
    let foreground = hex(scheme.foreground.as_str().unwrap_or_default(), WHITE);
    let foreground_rgb = hex_to_rgb(&foreground).unwrap_or_default();
    let ansi = ANSI_KEYS.map(|_| String::new());
    let mut palette = PaletteText {
        background: background.clone(),
        foreground: foreground.clone(),
        cursor: hex(scheme.cursor.as_str().unwrap_or_default(), &foreground),
        cursor_accent: OptionalText::Value(hex(
            scheme.cursor_text.as_str().unwrap_or_default(),
            &background,
        )),
        selection_background: match scheme
            .selection_background
            .as_str()
            .filter(|value| !value.is_empty())
        {
            Some(value) => hex(value, &foreground),
            None => format!(
                "rgba({}, {}, {}, 0.25)",
                foreground_rgb.r, foreground_rgb.g, foreground_rgb.b
            ),
        },
        black: ansi[0].clone(),
        red: ansi[1].clone(),
        green: ansi[2].clone(),
        yellow: ansi[3].clone(),
        blue: ansi[4].clone(),
        magenta: ansi[5].clone(),
        cyan: ansi[6].clone(),
        white: ansi[7].clone(),
        bright_black: ansi[8].clone(),
        bright_red: ansi[9].clone(),
        bright_green: ansi[10].clone(),
        bright_yellow: ansi[11].clone(),
        bright_blue: ansi[12].clone(),
        bright_magenta: ansi[13].clone(),
        bright_cyan: ansi[14].clone(),
        bright_white: ansi[15].clone(),
        selection_foreground: match scheme
            .selection_foreground
            .as_str()
            .filter(|value| !value.is_empty())
        {
            Some(value) => OptionalText::Value(hex(value, &foreground)),
            None => OptionalText::Missing,
        },
        extra: Default::default(),
    };
    for (index, key) in ANSI_KEYS.iter().enumerate() {
        let value = scheme
            .palette
            .get(index)
            .and_then(|value| value.as_deref())
            .unwrap_or_default();
        let fallback = if index < 8 { &background } else { &foreground };
        set_ansi(&mut palette, key, hex(value, fallback));
    }
    palette
}

/// Derives interface colors from a terminal palette, like
/// `deriveThemeColors` in `src/lib/themeDerivation.ts`.
pub fn derive_theme_colors(palette: &PaletteText) -> ThemeColors {
    let background = hex(&palette.background, BLACK);
    let palette_foreground = hex(&palette.foreground, WHITE);
    let depth = if is_dark_palette(palette) { 1.0 } else { 0.6 };
    let surface = |amount: f64| mix(&background, &palette_foreground, amount * depth);
    let muted = surface(0.13);
    let away = if depth == 1.0 { WHITE } else { BLACK };
    let foreground = ensure_contrast(
        &ensure_contrast(&palette_foreground, &background, 4.5, Some(away)),
        &muted,
        4.5,
        Some(away),
    );

    let primary = pick_primary(palette, &background);
    let red = ensure_contrast(&hex(&palette.red, "#e5534b"), &background, 3.0, None);
    let green = ensure_contrast(&hex(&palette.green, "#57ab5a"), &background, 3.0, None);
    let yellow = ensure_contrast(&hex(&palette.yellow, "#c69026"), &background, 3.0, None);
    let status_text =
        |color: &str| ensure_contrast(&mix(color, &foreground, 0.35), &background, 4.5, None);

    let mut tint = 0.22;
    while tint > 0.04 && contrast(&foreground, &mix(&background, &primary, tint)) < 4.5 {
        tint -= 0.03;
    }
    let accent = mix(&background, &primary, tint);

    ThemeColors {
        background: hsl_token(&background),
        foreground: hsl_token(&foreground),
        card: hsl_token(&surface(0.06)),
        card_foreground: hsl_token(&foreground),
        popover: hsl_token(&surface(0.07)),
        popover_foreground: hsl_token(&foreground),
        primary: hsl_token(&primary),
        primary_foreground: hsl_token(&text_on(&primary, &foreground, &background)),
        secondary: hsl_token(&surface(0.08)),
        secondary_foreground: hsl_token(&foreground),
        muted: hsl_token(&muted),
        muted_foreground: hsl_token(&ensure_contrast(
            &mix(&foreground, &background, 0.35),
            &surface(0.08),
            4.5,
            None,
        )),
        accent: hsl_token(&accent),
        accent_foreground: hsl_token(&foreground),
        destructive: hsl_token(&red),
        destructive_foreground: hsl_token(&text_on(&red, &foreground, &background)),
        success: hsl_token(&green),
        success_foreground: hsl_token(&status_text(&green)),
        warning: hsl_token(&yellow),
        warning_foreground: hsl_token(&status_text(&yellow)),
        border: hsl_token(&surface(0.16)),
        input: hsl_token(&surface(0.18)),
        ring: hsl_token(&primary),
        tab_background: hsl_token(&background),
        tab_active: hsl_token(&surface(0.10)),
        tab_active_border: hsl_token(&primary),
        tab_hover: hsl_token(&surface(0.14)),
        titlebar: hsl_token(&surface(0.04)),
        extra: Default::default(),
    }
}

fn set_ansi(palette: &mut PaletteText, key: &str, value: String) {
    match key {
        "black" => palette.black = value,
        "red" => palette.red = value,
        "green" => palette.green = value,
        "yellow" => palette.yellow = value,
        "blue" => palette.blue = value,
        "magenta" => palette.magenta = value,
        "cyan" => palette.cyan = value,
        "white" => palette.white = value,
        "brightBlack" => palette.bright_black = value,
        "brightRed" => palette.bright_red = value,
        "brightGreen" => palette.bright_green = value,
        "brightYellow" => palette.bright_yellow = value,
        "brightBlue" => palette.bright_blue = value,
        "brightMagenta" => palette.bright_magenta = value,
        "brightCyan" => palette.bright_cyan = value,
        "brightWhite" => palette.bright_white = value,
        _ => {}
    }
}

fn hex(value: &str, fallback: &str) -> String {
    if !value.is_empty()
        && let Some(hex) = color_to_hex(value)
    {
        return hex;
    }
    fallback.to_string()
}

fn mix(from: &str, to: &str, amount: f64) -> String {
    let from = hex_to_rgb(from).unwrap_or_default();
    let to = hex_to_rgb(to).unwrap_or_default();
    rgb_to_hex(
        f64::from(from.r) + (f64::from(to.r) - f64::from(from.r)) * amount,
        f64::from(from.g) + (f64::from(to.g) - f64::from(from.g)) * amount,
        f64::from(from.b) + (f64::from(to.b) - f64::from(from.b)) * amount,
    )
}

fn contrast(first: &str, second: &str) -> f64 {
    contrast_ratio(first, second).unwrap_or(1.0)
}

fn ensure_contrast(color: &str, background: &str, ratio: f64, target: Option<&str>) -> String {
    if contrast(color, background) >= ratio {
        return color.to_string();
    }
    let target = target.map(str::to_string).unwrap_or_else(|| {
        if contrast(WHITE, background) >= contrast(BLACK, background) {
            WHITE.to_string()
        } else {
            BLACK.to_string()
        }
    });
    for step in 1..=20 {
        let candidate = mix(color, &target, f64::from(step) / 20.0);
        if contrast(&candidate, background) >= ratio {
            return candidate;
        }
    }
    target
}

fn text_on(fill: &str, foreground: &str, background: &str) -> String {
    let mut candidates = [
        (foreground, contrast(foreground, fill)),
        (background, contrast(background, fill)),
    ];
    candidates.sort_by(|left, right| right.1.total_cmp(&left.1));
    let best = candidates
        .first()
        .map(|candidate| candidate.0)
        .unwrap_or(foreground);
    if contrast(best, fill) >= 4.5 {
        return best.to_string();
    }
    if contrast(WHITE, fill) >= contrast(BLACK, fill) {
        WHITE.to_string()
    } else {
        BLACK.to_string()
    }
}

fn chroma(color: &str) -> f64 {
    let color = hex_to_rgb(color).unwrap_or_default();
    f64::from(color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b)) / 255.0
}

fn pick_primary(palette: &PaletteText, background: &str) -> String {
    let candidates = [
        &palette.blue,
        &palette.bright_blue,
        &palette.cursor,
        &palette.magenta,
        &palette.cyan,
        &palette.bright_magenta,
        &palette.bright_cyan,
    ]
    .map(|color| hex(color, BLACK));
    if let Some(fitting) = candidates
        .iter()
        .find(|color| chroma(color) >= 0.2 && contrast(color, background) >= 2.5)
    {
        return fitting.clone();
    }

    let mut colorful = [
        &palette.red,
        &palette.green,
        &palette.yellow,
        &palette.blue,
        &palette.magenta,
        &palette.cyan,
        &palette.bright_red,
        &palette.bright_green,
        &palette.bright_yellow,
        &palette.bright_blue,
        &palette.bright_magenta,
        &palette.bright_cyan,
    ]
    .map(|color| hex(color, BLACK));
    colorful.sort_by(|left, right| chroma(right).total_cmp(&chroma(left)));
    let most_colorful = colorful
        .first()
        .cloned()
        .unwrap_or_else(|| BLACK.to_string());
    ensure_contrast(&most_colorful, background, 3.0, None)
}

fn hsl_token(color: &str) -> String {
    let color = hex_to_rgb(color).unwrap_or_default();
    let red = f64::from(color.r) / 255.0;
    let green = f64::from(color.g) / 255.0;
    let blue = f64::from(color.b) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    let mut hue = 0.0;
    let mut saturation = 0.0;
    if delta > 0.0 {
        saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
        if max == red {
            hue = ((green - blue) / delta + 6.0) % 6.0;
        } else if max == green {
            hue = (blue - red) / delta + 2.0;
        } else {
            hue = (red - green) / delta + 4.0;
        }
    }
    let round = |value: f64| js_round(value * 100.0) / 100.0;
    format!(
        "{} {}% {}%",
        js_number_to_string(round(hue * 60.0)),
        js_number_to_string(round(saturation * 100.0)),
        js_number_to_string(round(lightness * 100.0))
    )
}

#[cfg(test)]
mod tests;
