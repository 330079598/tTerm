//! The terminal's colors, and how a theme turns into them.
//!
//! The model keeps colors in their semantic form ([`crate::color::Color`]);
//! the palette is the bridge to pixels. It mirrors the resolved form of
//! `TerminalPalette` in `src/types/theme.ts`, including the optional cursor
//! text and selection text colors. The string form the frontend persists is
//! [`crate::theme::PaletteText`]; this type is what drawing reads.
//!
//! Two constructors matter:
//!
//! - [`TerminalPalette::default`] is the palette of the built-in `default`
//!   theme in `src/lib/themeDefinitions.ts`, which is what a first launch
//!   draws with.
//! - [`TerminalPalette::from_theme`] derives a palette from the interface
//!   colors, exactly like `generateTerminalPaletteFromColors` in
//!   `src/lib/terminalPalette.ts` does for a theme that has no terminal
//!   palette of its own.
//!
//! Both are checked against the TypeScript side in
//! `src/lib/terminalPalette.test.ts`; the Rust tests use the same expected
//! values.

use crate::color::{Color, NamedColor, Rgb};
use crate::theme::{PaletteText, ThemeColors};

/// An 8-bit sRGB color with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; 255 is opaque.
    pub a: u8,
}

impl Rgba {
    /// Builds a color from its four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Builds an opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// The same color at another alpha.
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { a: alpha, ..self }
    }

    /// Linearly blends toward `other`; `amount` 0 keeps `self` and 1 gives
    /// `other`.
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let blend = |from: u8, to: u8| {
            (f32::from(from) + (f32::from(to) - f32::from(from)) * amount).round() as u8
        };
        Self {
            r: blend(self.r, other.r),
            g: blend(self.g, other.g),
            b: blend(self.b, other.b),
            a: blend(self.a, other.a),
        }
    }

    /// The color as `#rrggbb`, for tests and logs.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Parses `#rrggbb`, `#rgb` or `rgba(r, g, b, a)`.
    ///
    /// Only the forms the built-in theme uses are accepted: the function
    /// returns `None` instead of guessing, so a bad theme falls back to the
    /// default palette rather than to an arbitrary color.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if let Some(hex) = value.strip_prefix('#') {
            let expanded = match hex.len() {
                3 => hex
                    .chars()
                    .flat_map(|character| [character, character])
                    .collect::<String>(),
                6 => hex.to_string(),
                _ => return None,
            };
            if !expanded
                .chars()
                .all(|character| character.is_ascii_hexdigit())
            {
                return None;
            }
            let channel = |index: usize| u8::from_str_radix(&expanded[index..index + 2], 16).ok();
            return Some(Self::rgb(channel(0)?, channel(2)?, channel(4)?));
        }

        let inner = value
            .strip_prefix("rgba(")
            .or_else(|| value.strip_prefix("rgb("))?
            .strip_suffix(')')?;
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() < 3 {
            return None;
        }
        let channel = |index: usize| parts[index].parse::<u8>().ok();
        let alpha = match parts.get(3) {
            Some(text) => (text.parse::<f32>().ok()?.clamp(0.0, 1.0) * 255.0).round() as u8,
            None => 255,
        };
        Some(Self::new(channel(0)?, channel(1)?, channel(2)?, alpha))
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::rgb(0, 0, 0)
    }
}

/// The colors a terminal draws with.
///
/// The fields are the ones `TerminalPalette` declares in
/// `src/types/theme.ts`. There is deliberately no search-highlight field:
/// the TypeScript type has none either. [`TerminalPalette::search_match`] and
/// [`TerminalPalette::search_active`] derive overlay colors for P2-07.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TerminalPalette {
    /// The default background.
    pub background: Rgba,
    /// The default foreground.
    pub foreground: Rgba,
    /// The cursor color.
    pub cursor: Rgba,
    /// Text drawn inside a block cursor, when the theme sets one.
    pub cursor_accent: Option<Rgba>,
    /// The selection background, usually translucent.
    pub selection_background: Rgba,
    /// Selected text, when the theme overrides the cell's own color.
    pub selection_foreground: Option<Rgba>,
    /// ANSI black.
    pub black: Rgba,
    /// ANSI red.
    pub red: Rgba,
    /// ANSI green.
    pub green: Rgba,
    /// ANSI yellow.
    pub yellow: Rgba,
    /// ANSI blue.
    pub blue: Rgba,
    /// ANSI magenta.
    pub magenta: Rgba,
    /// ANSI cyan.
    pub cyan: Rgba,
    /// ANSI white.
    pub white: Rgba,
    /// ANSI bright black.
    pub bright_black: Rgba,
    /// ANSI bright red.
    pub bright_red: Rgba,
    /// ANSI bright green.
    pub bright_green: Rgba,
    /// ANSI bright yellow.
    pub bright_yellow: Rgba,
    /// ANSI bright blue.
    pub bright_blue: Rgba,
    /// ANSI bright magenta.
    pub bright_magenta: Rgba,
    /// ANSI bright cyan.
    pub bright_cyan: Rgba,
    /// ANSI bright white.
    pub bright_white: Rgba,
}

impl Default for TerminalPalette {
    fn default() -> Self {
        // The explicit `terminal` object of PRESET_THEMES[0] in
        // src/lib/themeDefinitions.ts.
        Self {
            background: Rgba::rgb(0x1b, 0x1d, 0x23),
            foreground: Rgba::rgb(0xd7, 0xdd, 0xe8),
            cursor: Rgba::rgb(0x79, 0xd8, 0xff),
            cursor_accent: None,
            selection_background: Rgba::new(77, 182, 255, 56),
            selection_foreground: None,
            black: Rgba::rgb(0x16, 0x1b, 0x22),
            red: Rgba::rgb(0xf1, 0x6c, 0x75),
            green: Rgba::rgb(0x7b, 0xd8, 0x8f),
            yellow: Rgba::rgb(0xe6, 0xc1, 0x6a),
            blue: Rgba::rgb(0x6a, 0xae, 0xff),
            magenta: Rgba::rgb(0xc7, 0x92, 0xea),
            cyan: Rgba::rgb(0x66, 0xd9, 0xef),
            white: Rgba::rgb(0xc8, 0xd2, 0xdc),
            bright_black: Rgba::rgb(0x5c, 0x67, 0x73),
            bright_red: Rgba::rgb(0xff, 0x8b, 0x94),
            bright_green: Rgba::rgb(0x9e, 0xe6, 0xb8),
            bright_yellow: Rgba::rgb(0xf2, 0xd3, 0x8f),
            bright_blue: Rgba::rgb(0x8f, 0xc7, 0xff),
            bright_magenta: Rgba::rgb(0xdd, 0xb7, 0xff),
            bright_cyan: Rgba::rgb(0x8b, 0xe9, 0xfd),
            bright_white: Rgba::rgb(0xf4, 0xf7, 0xfb),
        }
    }
}

impl TerminalPalette {
    /// Derives a palette the way the theme editor does.
    ///
    /// This is `generateTerminalPaletteFromColors` from
    /// `src/lib/terminalPalette.ts`, with the CSS `hsl()` values converted to
    /// sRGB. The tests compare every field with the TypeScript result.
    pub fn from_theme(colors: &ThemeColors) -> Self {
        PaletteText::from_theme_colors(colors).to_terminal_palette()
    }

    /// The color a search match is highlighted with, for P2-07.
    pub fn search_match(&self) -> Rgba {
        self.yellow.with_alpha(89)
    }

    /// The color the active search match is highlighted with, for P2-07.
    pub fn search_active(&self) -> Rgba {
        self.yellow.with_alpha(153)
    }

    /// Resolves a cell's semantic foreground color.
    ///
    /// Bold text uses the bright variant of the eight standard colors, as
    /// xterm.js does by default; explicit bright, dim, indexed, true and
    /// special colors are left alone.
    pub fn resolve(&self, color: Color, bold: bool) -> Rgba {
        match color {
            Color::Named(named) => self.named(named, bold),
            Color::Indexed(index) => self.indexed(index),
            Color::Spec(Rgb { r, g, b }) => Rgba::rgb(r, g, b),
        }
    }

    /// One of the 256 indexed colors.
    pub fn indexed(&self, index: u8) -> Rgba {
        match index {
            0 => self.black,
            1 => self.red,
            2 => self.green,
            3 => self.yellow,
            4 => self.blue,
            5 => self.magenta,
            6 => self.cyan,
            7 => self.white,
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            16..=231 => {
                const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
                let index = index as usize - 16;
                let red = LEVELS[index / 36];
                let green = LEVELS[(index % 36) / 6];
                let blue = LEVELS[index % 6];
                Rgba::rgb(red, green, blue)
            }
            _ => {
                let level = 8 + 10 * (index as u16 - 232);
                Rgba::rgb(level as u8, level as u8, level as u8)
            }
        }
    }

    fn named(&self, named: NamedColor, bold: bool) -> Rgba {
        use NamedColor::*;
        let standard = match named {
            Black | DimBlack => self.black,
            Red | DimRed => self.red,
            Green | DimGreen => self.green,
            Yellow | DimYellow => self.yellow,
            Blue | DimBlue => self.blue,
            Magenta | DimMagenta => self.magenta,
            Cyan | DimCyan => self.cyan,
            White | DimWhite => self.white,
            BrightBlack => self.bright_black,
            BrightRed => self.bright_red,
            BrightGreen => self.bright_green,
            BrightYellow => self.bright_yellow,
            BrightBlue => self.bright_blue,
            BrightMagenta => self.bright_magenta,
            BrightCyan => self.bright_cyan,
            BrightWhite => self.bright_white,
            Foreground | BrightForeground => self.foreground,
            Background => self.background,
            Cursor => self.cursor,
            DimForeground => return self.foreground.mix(self.background, 0.5),
        };

        let dimmed = matches!(
            named,
            DimBlack | DimRed | DimGreen | DimYellow | DimBlue | DimMagenta | DimCyan | DimWhite
        );
        if dimmed {
            return standard.mix(self.background, 0.5);
        }
        if bold {
            return match named {
                Black => self.bright_black,
                Red => self.bright_red,
                Green => self.bright_green,
                Yellow => self.bright_yellow,
                Blue => self.bright_blue,
                Magenta => self.bright_magenta,
                Cyan => self.bright_cyan,
                White => self.bright_white,
                _ => standard,
            };
        }
        standard
    }
}

#[cfg(test)]
mod tests {
    use crate::theme::OptionalText;

    use super::*;

    fn hex(value: &str) -> Rgba {
        Rgba::parse(value).expect("the test writes a color")
    }

    #[test]
    fn default_palette_matches_the_default_theme() {
        let palette = TerminalPalette::default();
        assert_eq!(palette.background.to_hex(), "#1b1d23");
        assert_eq!(palette.foreground.to_hex(), "#d7dde8");
        assert_eq!(palette.cursor.to_hex(), "#79d8ff");
        assert_eq!(palette.cursor_accent, None);
        assert_eq!(palette.selection_background, Rgba::new(77, 182, 255, 56));
        assert_eq!(palette.selection_foreground, None);
        assert_eq!(
            [
                palette.black,
                palette.red,
                palette.green,
                palette.yellow,
                palette.blue,
                palette.magenta,
                palette.cyan,
                palette.white,
                palette.bright_black,
                palette.bright_red,
                palette.bright_green,
                palette.bright_yellow,
                palette.bright_blue,
                palette.bright_magenta,
                palette.bright_cyan,
                palette.bright_white,
            ]
            .map(Rgba::to_hex),
            [
                "#161b22", "#f16c75", "#7bd88f", "#e6c16a", "#6aaeff", "#c792ea", "#66d9ef",
                "#c8d2dc", "#5c6773", "#ff8b94", "#9ee6b8", "#f2d38f", "#8fc7ff", "#ddb7ff",
                "#8be9fd", "#f4f7fb",
            ]
        );
    }

    #[test]
    fn derived_palette_matches_the_typescript_derivation() {
        let text = PaletteText::from_theme_colors(&ThemeColors::dark_default());
        assert_eq!(text.selection_background, "hsl(207 100% 40% / 0.24)");
        assert_eq!(text.cursor_accent, OptionalText::Missing);
        assert_eq!(text.selection_foreground, OptionalText::Missing);
        let palette = TerminalPalette::from_theme(&ThemeColors::dark_default());
        assert_eq!(
            [
                palette.background,
                palette.foreground,
                palette.cursor,
                palette.black,
                palette.red,
                palette.green,
                palette.yellow,
                palette.blue,
                palette.magenta,
                palette.cyan,
                palette.white,
                palette.bright_black,
                palette.bright_red,
                palette.bright_green,
                palette.bright_yellow,
                palette.bright_blue,
                palette.bright_magenta,
                palette.bright_cyan,
                palette.bright_white,
            ]
            .map(Rgba::to_hex),
            [
                "#1b1d23", "#cccccc", "#005aa3", "#16181d", "#ef4343", "#1eb857", "#f5a314",
                "#0070cc", "#5c07ed", "#0070cc", "#bdbdbd", "#1d2025", "#eb1e1e", "#1a9e4a",
                "#d78c09", "#005aa3", "#4c06c6", "#005aa3", "#4f4f4f",
            ]
        );
        assert_eq!(
            palette.selection_background,
            Rgba::new(0x00, 0x70, 0xcc, 61)
        );
    }

    #[test]
    fn indexed_colors_follow_the_xterm_cube() {
        let palette = TerminalPalette::default();
        assert_eq!(palette.indexed(0), palette.black);
        assert_eq!(palette.indexed(15), palette.bright_white);
        assert_eq!(palette.indexed(16), Rgba::rgb(0, 0, 0));
        assert_eq!(palette.indexed(21), Rgba::rgb(0, 0, 255));
        assert_eq!(palette.indexed(196), Rgba::rgb(255, 0, 0));
        assert_eq!(palette.indexed(231), Rgba::rgb(255, 255, 255));
        assert_eq!(palette.indexed(232), Rgba::rgb(8, 8, 8));
        assert_eq!(palette.indexed(255), Rgba::rgb(238, 238, 238));
    }

    #[test]
    fn bold_standard_colors_use_the_bright_variant() {
        let palette = TerminalPalette::default();
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::Red), false),
            palette.red
        );
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::Red), true),
            palette.bright_red
        );
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::BrightRed), true),
            palette.bright_red
        );
        assert_eq!(
            palette.resolve(Color::Spec(Rgb { r: 1, g: 2, b: 3 }), true),
            Rgba::rgb(1, 2, 3)
        );
    }

    #[test]
    fn special_and_dim_colors_resolve() {
        let palette = TerminalPalette::default();
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::Foreground), false),
            palette.foreground
        );
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::Background), false),
            palette.background
        );
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::Cursor), false),
            palette.cursor
        );
        assert_eq!(
            palette.resolve(Color::Named(NamedColor::DimForeground), false),
            palette.foreground.mix(palette.background, 0.5)
        );
    }

    #[test]
    fn colors_parse_the_forms_the_theme_uses() {
        assert_eq!(hex("#1b1d23"), Rgba::rgb(0x1b, 0x1d, 0x23));
        assert_eq!(hex("#abc"), Rgba::rgb(0xaa, 0xbb, 0xcc));
        assert_eq!(
            Rgba::parse("rgba(77, 182, 255, 0.22)"),
            Some(Rgba::new(77, 182, 255, 56))
        );
        assert_eq!(Rgba::parse("not a color"), None);
    }

    #[test]
    fn search_highlights_are_semitransparent_yellow() {
        let palette = TerminalPalette::default();
        assert_eq!(palette.search_match(), palette.yellow.with_alpha(89));
        assert_eq!(palette.search_active(), palette.yellow.with_alpha(153));
        assert!(palette.search_active().a > palette.search_match().a);
    }
}
