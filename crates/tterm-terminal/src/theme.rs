//! Theme colors, their persisted text form, and the color math the theme
//! editor and the theme importer share.
//!
//! [`ThemeColors`] mirrors `ThemeColors` in `src/types/theme.ts`: the
//! twenty-eight HSL tokens the interface reads. [`PaletteText`] mirrors the
//! string form of `TerminalPalette`, which is what the frontend persists and
//! what theme files contain. [`crate::palette::TerminalPalette`] is the
//! resolved drawing form; [`PaletteText::to_terminal_palette`] is the bridge.
//!
//! Keeping the text form separate matters for more than typing: a saved
//! theme writes `hsl(220 13% 12%)` where the drawing form only has an RGB
//! triple. Serializing the drawing form would change the JSON the Tauri
//! backend already stores.
//!
//! The functions here follow `src/components/ThemeEditor/colorUtils.ts` and
//! `src/lib/terminalPalette.ts`. They use the ECMAScript regular expression
//! engine (`regress`) where the TypeScript side uses a `RegExp`, because
//! `\s`, `\d` and case folding differ between the two dialects.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regress::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::palette::{Rgba, TerminalPalette};

/// JavaScript-compatible string and number helpers.
pub mod js;

mod css;

use self::css::parse_palette_color;
use self::js::{js_number_to_string, js_round, js_trim};

/// The interface color keys, in the order the frontend serializes them.
pub const THEME_COLOR_KEYS: [&str; 28] = [
    "background",
    "foreground",
    "card",
    "cardForeground",
    "popover",
    "popoverForeground",
    "primary",
    "primaryForeground",
    "secondary",
    "secondaryForeground",
    "muted",
    "mutedForeground",
    "accent",
    "accentForeground",
    "destructive",
    "destructiveForeground",
    "success",
    "successForeground",
    "warning",
    "warningForeground",
    "border",
    "input",
    "ring",
    "tabBackground",
    "tabActive",
    "tabActiveBorder",
    "tabHover",
    "titlebar",
];

/// The terminal palette keys, in the order the frontend creates them.
pub const TERMINAL_PALETTE_KEYS: [&str; 22] = [
    "background",
    "foreground",
    "cursor",
    "cursorAccent",
    "selectionBackground",
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
    "selectionForeground",
];

/// An optional field that distinguishes "absent" from a JSON `null`.
///
/// `src/types/theme.ts` declares these fields optional, while
/// `normalizeCustomTheme` copies whatever it finds, including `null` and a
/// value of an unexpected type. The states are kept apart so a round trip
/// through the backend does not change what the web view stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OptionalText {
    /// The property is absent from the JSON object.
    #[default]
    Missing,
    /// The property is present with the value `null`.
    Null,
    /// The property is a string.
    Value(String),
    /// The property is present with a non-string value the web view kept.
    Json(Value),
}

impl OptionalText {
    /// Returns the text when the property is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null | Self::Json(_) => None,
        }
    }

    /// Whether the property should be omitted while serializing.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl Serialize for OptionalText {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Missing => serializer.serialize_none(),
            Self::Null => serializer.serialize_none(),
            Self::Value(value) => serializer.serialize_str(value),
            Self::Json(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OptionalText {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Value::deserialize(deserializer)? {
            Value::Null => Self::Null,
            Value::String(value) => Self::Value(value),
            value => Self::Json(value),
        })
    }
}

impl From<&str> for OptionalText {
    fn from(value: &str) -> Self {
        Self::Value(value.to_string())
    }
}

impl From<String> for OptionalText {
    fn from(value: String) -> Self {
        Self::Value(value)
    }
}

/// The interface colors of a theme, as the HSL tokens the stylesheet uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    /// The interface background.
    pub background: String,
    /// The default text color.
    pub foreground: String,
    /// The raised surface.
    pub card: String,
    /// Text on [`ThemeColors::card`].
    pub card_foreground: String,
    /// The popover surface.
    pub popover: String,
    /// Text on [`ThemeColors::popover`].
    pub popover_foreground: String,
    /// The primary accent.
    pub primary: String,
    /// Text on [`ThemeColors::primary`].
    pub primary_foreground: String,
    /// The secondary accent.
    pub secondary: String,
    /// Text on [`ThemeColors::secondary`].
    pub secondary_foreground: String,
    /// The muted surface.
    pub muted: String,
    /// Text on [`ThemeColors::muted`].
    pub muted_foreground: String,
    /// The hovered and selected surface.
    pub accent: String,
    /// Text on [`ThemeColors::accent`].
    pub accent_foreground: String,
    /// The destructive action color.
    pub destructive: String,
    /// Text on [`ThemeColors::destructive`].
    pub destructive_foreground: String,
    /// The success color.
    pub success: String,
    /// Text on the success color.
    pub success_foreground: String,
    /// The warning color.
    pub warning: String,
    /// Text on the warning color.
    pub warning_foreground: String,
    /// The default border.
    pub border: String,
    /// The input border.
    pub input: String,
    /// The focus ring.
    pub ring: String,
    /// The tab strip background.
    pub tab_background: String,
    /// The active tab.
    pub tab_active: String,
    /// The active tab's border.
    pub tab_active_border: String,
    /// A hovered tab.
    pub tab_hover: String,
    /// The title bar.
    pub titlebar: String,
    /// Color keys a newer frontend wrote, kept so they survive a save.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

impl ThemeColors {
    /// The colors of the built-in `default` theme.
    pub fn dark_default() -> Self {
        Self {
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
            extra: BTreeMap::new(),
        }
    }
}

/// A terminal palette in the string form the frontend persists.
///
/// The field order is the order the frontend creates its objects in, so
/// serializing an imported palette produces the same JSON key order as the
/// TypeScript side. `selectionForeground` is last because
/// `terminalPaletteFromScheme` assigns it after the sixteen ANSI colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PaletteText {
    /// The default background.
    pub background: String,
    /// The default foreground.
    pub foreground: String,
    /// The cursor color.
    pub cursor: String,
    /// Text drawn inside a block cursor.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub cursor_accent: OptionalText,
    /// The selection background.
    pub selection_background: String,
    /// ANSI black.
    pub black: String,
    /// ANSI red.
    pub red: String,
    /// ANSI green.
    pub green: String,
    /// ANSI yellow.
    pub yellow: String,
    /// ANSI blue.
    pub blue: String,
    /// ANSI magenta.
    pub magenta: String,
    /// ANSI cyan.
    pub cyan: String,
    /// ANSI white.
    pub white: String,
    /// ANSI bright black.
    pub bright_black: String,
    /// ANSI bright red.
    pub bright_red: String,
    /// ANSI bright green.
    pub bright_green: String,
    /// ANSI bright yellow.
    pub bright_yellow: String,
    /// ANSI bright blue.
    pub bright_blue: String,
    /// ANSI bright magenta.
    pub bright_magenta: String,
    /// ANSI bright cyan.
    pub bright_cyan: String,
    /// ANSI bright white.
    pub bright_white: String,
    /// Selected text, when the theme overrides the cell's own color.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub selection_foreground: OptionalText,
    /// Terminal color keys a newer frontend wrote.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for PaletteText {
    fn default() -> Self {
        Self {
            background: String::new(),
            foreground: String::new(),
            cursor: String::new(),
            cursor_accent: OptionalText::Missing,
            selection_background: String::new(),
            black: String::new(),
            red: String::new(),
            green: String::new(),
            yellow: String::new(),
            blue: String::new(),
            magenta: String::new(),
            cyan: String::new(),
            white: String::new(),
            bright_black: String::new(),
            bright_red: String::new(),
            bright_green: String::new(),
            bright_yellow: String::new(),
            bright_blue: String::new(),
            bright_magenta: String::new(),
            bright_cyan: String::new(),
            bright_white: String::new(),
            selection_foreground: OptionalText::Missing,
            extra: BTreeMap::new(),
        }
    }
}

impl PaletteText {
    /// Derives a palette from interface colors, like
    /// `generateTerminalPaletteFromColors` in `src/lib/terminalPalette.ts`.
    pub fn from_theme_colors(colors: &ThemeColors) -> Self {
        let surface = parse_hsl(&colors.background).unwrap_or(Hsl::new(220.0, 13.0, 12.0));
        let foreground = parse_hsl(&colors.foreground).unwrap_or(Hsl::new(0.0, 0.0, 88.0));
        let accent = parse_hsl(&colors.primary)
            .or_else(|| parse_hsl(&colors.accent))
            .unwrap_or(Hsl::new(207.0, 100.0, 40.0));
        let success = parse_hsl(&colors.success).unwrap_or(Hsl::new(142.0, 72.0, 42.0));
        let warning = parse_hsl(&colors.warning).unwrap_or(Hsl::new(38.0, 92.0, 52.0));
        let destructive = parse_hsl(&colors.destructive).unwrap_or(Hsl::new(0.0, 84.0, 60.0));
        let neutral = parse_hsl(&colors.muted).unwrap_or_else(|| {
            shift(
                surface,
                0.0,
                0.0,
                if foreground.l > 50.0 { 12.0 } else { 18.0 },
            )
        });
        let neutral_text = parse_hsl(&colors.muted_foreground).unwrap_or_else(|| {
            shift(
                foreground,
                0.0,
                0.0,
                if foreground.l > 50.0 { -18.0 } else { 18.0 },
            )
        });
        let cyan_base =
            parse_hsl(&colors.accent).unwrap_or_else(|| shift(accent, -18.0, -8.0, 4.0));
        let magenta_base = shift(accent, 55.0, -6.0, 8.0);
        let light = foreground.l > 50.0;
        let white_base = shift(foreground, 0.0, -10.0, if light { -6.0 } else { 8.0 });

        Self {
            background: hsl_to_css(surface),
            foreground: hsl_to_css(foreground),
            cursor: hsl_to_css(shift(accent, 0.0, 0.0, if light { -8.0 } else { 14.0 })),
            cursor_accent: OptionalText::Missing,
            selection_background: hsla_to_css(accent, 0.24),
            black: hsl_to_css(shift(surface, 0.0, 0.0, if light { -2.0 } else { 8.0 })),
            red: hsl_to_css(destructive),
            green: hsl_to_css(success),
            yellow: hsl_to_css(warning),
            blue: hsl_to_css(accent),
            magenta: hsl_to_css(magenta_base),
            cyan: hsl_to_css(cyan_base),
            white: hsl_to_css(white_base),
            bright_black: hsl_to_css(shift(neutral, 0.0, 0.0, if light { -10.0 } else { 12.0 })),
            bright_red: hsl_to_css(shift(
                destructive,
                0.0,
                0.0,
                if light { -8.0 } else { 10.0 },
            )),
            bright_green: hsl_to_css(shift(success, 0.0, 0.0, if light { -6.0 } else { 10.0 })),
            bright_yellow: hsl_to_css(shift(warning, 0.0, 0.0, if light { -8.0 } else { 8.0 })),
            bright_blue: hsl_to_css(shift(accent, 0.0, 0.0, if light { -8.0 } else { 12.0 })),
            bright_magenta: hsl_to_css(shift(
                magenta_base,
                0.0,
                0.0,
                if light { -8.0 } else { 10.0 },
            )),
            bright_cyan: hsl_to_css(shift(cyan_base, 0.0, 0.0, if light { -8.0 } else { 10.0 })),
            bright_white: hsl_to_css(shift(
                neutral_text,
                0.0,
                -10.0,
                if light { -28.0 } else { 22.0 },
            )),
            selection_foreground: OptionalText::Missing,
            extra: BTreeMap::new(),
        }
    }

    /// Resolves the palette into the colors the terminal draws with.
    ///
    /// Invalid colors fall back to black; imported palettes are validated
    /// before they get here, and this is the only place a malformed saved
    /// theme can reach the drawing path.
    pub fn to_terminal_palette(&self) -> TerminalPalette {
        let color = |value: &str| parse_palette_color(value).unwrap_or_default();
        let optional = |value: &OptionalText| value.as_str().and_then(parse_palette_color);
        TerminalPalette {
            background: color(&self.background),
            foreground: color(&self.foreground),
            cursor: color(&self.cursor),
            cursor_accent: optional(&self.cursor_accent),
            selection_background: color(&self.selection_background),
            selection_foreground: optional(&self.selection_foreground),
            black: color(&self.black),
            red: color(&self.red),
            green: color(&self.green),
            yellow: color(&self.yellow),
            blue: color(&self.blue),
            magenta: color(&self.magenta),
            cyan: color(&self.cyan),
            white: color(&self.white),
            bright_black: color(&self.bright_black),
            bright_red: color(&self.bright_red),
            bright_green: color(&self.bright_green),
            bright_yellow: color(&self.bright_yellow),
            bright_blue: color(&self.bright_blue),
            bright_magenta: color(&self.bright_magenta),
            bright_cyan: color(&self.bright_cyan),
            bright_white: color(&self.bright_white),
        }
    }

    /// Copies the palette with a transparent background when the window blur
    /// is on, like `withWindowBlur` in `src/lib/terminalPalette.ts`.
    pub fn with_window_blur(&self, blur: bool) -> Self {
        if !blur {
            return self.clone();
        }
        let hex = color_to_hex(&self.background).unwrap_or_else(|| "#000000".into());
        Self {
            background: format!("{hex}00"),
            ..self.clone()
        }
    }

    /// Whether the background is the `#rrggbb00` form the blur setting writes.
    pub fn is_transparent(&self) -> bool {
        static TRANSPARENT: LazyLock<Option<Regex>> =
            LazyLock::new(|| Regex::with_flags(r"^#[\da-f]{6}00$", "i").ok());
        TRANSPARENT
            .as_ref()
            .is_some_and(|pattern| pattern.find(&self.background).is_some())
    }
}

/// A hue, saturation and lightness triple in the units stylesheets use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    /// Hue in degrees.
    pub h: f64,
    /// Saturation in percent.
    pub s: f64,
    /// Lightness in percent.
    pub l: f64,
}

impl Hsl {
    /// Builds an HSL triple.
    pub const fn new(h: f64, s: f64, l: f64) -> Self {
        Self { h, s, l }
    }
}

/// A color with fractional channels, as `hslToRgb` returns it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HslRgb {
    /// Red channel, 0 to 255.
    pub r: f64,
    /// Green channel, 0 to 255.
    pub g: f64,
    /// Blue channel, 0 to 255.
    pub b: f64,
}

/// Parses a CSS color the way `colorToHex` does.
///
/// Accepted forms are `#rgb`, `#rrggbb`, `rgb()`/`rgba()` and `hsl()`/
/// `hsla()`; the result is always lowercase `#rrggbb`.
pub fn color_to_hex(value: &str) -> Option<String> {
    static RGB_FUNCTION: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::with_flags(
            r"^rgba?\(\s*(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)[\s,]+(\d+(?:\.\d+)?)",
            "i",
        )
        .ok()
    });
    static HSL_TOKEN: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::with_flags(
            r"^(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)%\s+(\d+(?:\.\d+)?)%$",
            "",
        )
        .ok()
    });
    static HSL_FUNCTION: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::with_flags(
            r"^hsla?\(\s*(\d+(?:\.\d+)?)(?:deg)?[\s,]+(\d+(?:\.\d+)?)%[\s,]+(\d+(?:\.\d+)?)%",
            "i",
        )
        .ok()
    });

    let normalized = js_trim(value);
    if let Some(hex) = normalized.strip_prefix('#')
        && (hex.len() == 3 || hex.len() == 6)
        && hex.chars().all(|character| character.is_ascii_hexdigit())
    {
        return hex_to_rgb(normalized).map(|color| color.to_hex());
    }
    if let Some(pattern) = RGB_FUNCTION.as_ref()
        && let Some(captures) = pattern.find(normalized)
    {
        let channel = |index: usize| -> Option<f64> {
            let range = captures.group(index)?;
            normalized[range].parse().ok()
        };
        return Some(rgb_to_hex(channel(1)?, channel(2)?, channel(3)?));
    }
    let parts = HSL_TOKEN
        .as_ref()
        .and_then(|pattern| pattern.find(normalized))
        .or_else(|| {
            HSL_FUNCTION
                .as_ref()
                .and_then(|pattern| pattern.find(normalized))
        })?;
    let number = |index: usize| -> Option<f64> {
        let range = parts.group(index)?;
        normalized[range].parse().ok()
    };
    let rgb = hsl_to_rgb(number(1)?, number(2)?, number(3)?);
    Some(rgb_to_hex(rgb.r, rgb.g, rgb.b))
}

/// Parses `#rgb`, `#rrggbb` or a bare three/six digit hex color.
pub fn hex_to_rgb(hex: &str) -> Option<Rgba> {
    let normalized = js_trim(hex);
    let normalized = normalized.strip_prefix('#').unwrap_or(normalized);
    let expanded = if normalized.len() == 3 {
        normalized
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>()
    } else {
        normalized.to_string()
    };
    if expanded.len() != 6
        || !expanded
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    let channel = |index: usize| u8::from_str_radix(&expanded[index..index + 2], 16).ok();
    Some(Rgba::rgb(channel(0)?, channel(2)?, channel(4)?))
}

/// Formats fractional channels as `#rrggbb`, clamping and rounding like
/// `rgbToHex` in `src/components/ThemeEditor/colorUtils.ts`.
pub fn rgb_to_hex(red: f64, green: f64, blue: f64) -> String {
    let channel = |value: f64| {
        let value = js_round(value.clamp(0.0, 255.0)) as u8;
        format!("{value:02x}")
    };
    format!("#{}{}{}", channel(red), channel(green), channel(blue))
}

/// Converts HSL to fractional sRGB channels.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> HslRgb {
    let hue = (((h % 360.0) + 360.0) % 360.0) / 360.0;
    let saturation = s.clamp(0.0, 100.0) / 100.0;
    let lightness = l.clamp(0.0, 100.0) / 100.0;
    if saturation == 0.0 {
        let gray = lightness * 255.0;
        return HslRgb {
            r: gray,
            g: gray,
            b: gray,
        };
    }
    let upper = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let lower = 2.0 * lightness - upper;
    let channel = |mut shifted: f64| {
        if shifted < 0.0 {
            shifted += 1.0;
        }
        if shifted > 1.0 {
            shifted -= 1.0;
        }
        if shifted < 1.0 / 6.0 {
            lower + (upper - lower) * 6.0 * shifted
        } else if shifted < 1.0 / 2.0 {
            upper
        } else if shifted < 2.0 / 3.0 {
            lower + (upper - lower) * (2.0 / 3.0 - shifted) * 6.0
        } else {
            lower
        }
    };
    HslRgb {
        r: channel(hue + 1.0 / 3.0) * 255.0,
        g: channel(hue) * 255.0,
        b: channel(hue - 1.0 / 3.0) * 255.0,
    }
}

/// Parses the loose HSL token `parseHsl` in `src/lib/terminalPalette.ts`
/// accepts. It deliberately is not anchored and does not require percent
/// signs, matching the TypeScript regular expression.
pub fn parse_hsl(hsl: &str) -> Option<Hsl> {
    static PATTERN: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::with_flags(r"(\d+\.?\d*)\s+(\d+\.?\d*)%?\s+(\d+\.?\d*)%?", "").ok()
    });
    let normalized = js_trim(hsl);
    let pattern = PATTERN.as_ref()?;
    let captures = pattern.find(normalized)?;
    let number = |index: usize| -> Option<f64> {
        let range = captures.group(index)?;
        normalized[range].parse().ok()
    };
    Some(Hsl::new(number(1)?, number(2)?, number(3)?))
}

/// Relative luminance of a color, like `relativeLuminance` in `colorUtils.ts`.
pub fn relative_luminance(hex: &str) -> Option<f64> {
    let color = hex_to_rgb(hex)?;
    let channel = |value: u8| {
        let normalized = f64::from(value) / 255.0;
        if normalized <= 0.039_28 {
            normalized / 12.92
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b))
}

/// WCAG contrast ratio, like `contrastRatio` in `colorUtils.ts`.
pub fn contrast_ratio(first: &str, second: &str) -> Option<f64> {
    let first = relative_luminance(first)?;
    let second = relative_luminance(second)?;
    let lighter = first.max(second);
    let darker = first.min(second);
    Some((lighter + 0.05) / (darker + 0.05))
}

fn shift(base: Hsl, h: f64, s: f64, l: f64) -> Hsl {
    Hsl {
        h: (base.h + h + 360.0) % 360.0,
        s: (base.s + s).clamp(0.0, 100.0),
        l: (base.l + l).clamp(0.0, 100.0),
    }
}

fn hsl_to_css(color: Hsl) -> String {
    format!(
        "hsl({} {}% {}%)",
        js_number_to_string(color.h),
        js_number_to_string(color.s),
        js_number_to_string(color.l)
    )
}

fn hsla_to_css(color: Hsl, alpha: f64) -> String {
    format!(
        "hsl({} {}% {}% / {})",
        js_number_to_string(color.h),
        js_number_to_string(color.s),
        js_number_to_string(color.l),
        js_number_to_string(alpha)
    )
}

#[cfg(test)]
mod tests;
