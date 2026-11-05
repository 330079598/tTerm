//! Colors of one cell, kept in the semantic form the application chose.
//!
//! A snapshot never resolves a color to RGB: it records the named color, the
//! palette index or the 24-bit value the program asked for. The view maps it
//! through the current theme while drawing, so changing a theme redraws the
//! same frame instead of reparsing the session (design note 4.1).

use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor as AnsiNamedColor};

/// A 24-bit color that the application specified itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// A color the terminal protocol names, or the theme supplies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NamedColor {
    /// The default black.
    Black,
    /// The default red.
    Red,
    /// The default green.
    Green,
    /// The default yellow.
    Yellow,
    /// The default blue.
    Blue,
    /// The default magenta.
    Magenta,
    /// The default cyan.
    Cyan,
    /// The default white.
    White,
    /// The bright variant of black.
    BrightBlack,
    /// The bright variant of red.
    BrightRed,
    /// The bright variant of green.
    BrightGreen,
    /// The bright variant of yellow.
    BrightYellow,
    /// The bright variant of blue.
    BrightBlue,
    /// The bright variant of magenta.
    BrightMagenta,
    /// The bright variant of cyan.
    BrightCyan,
    /// The bright variant of white.
    BrightWhite,
    /// The default foreground.
    Foreground,
    /// The default background.
    Background,
    /// The cursor color.
    Cursor,
    /// The dim variant of black.
    DimBlack,
    /// The dim variant of red.
    DimRed,
    /// The dim variant of green.
    DimGreen,
    /// The dim variant of yellow.
    DimYellow,
    /// The dim variant of blue.
    DimBlue,
    /// The dim variant of magenta.
    DimMagenta,
    /// The dim variant of cyan.
    DimCyan,
    /// The dim variant of white.
    DimWhite,
    /// The bright variant of the default foreground.
    BrightForeground,
    /// The dim variant of the default foreground.
    DimForeground,
}

/// A color of one cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// One of the colors the theme defines.
    Named(NamedColor),
    /// An index into the 256-color palette.
    Indexed(u8),
    /// A 24-bit color.
    Spec(Rgb),
}

impl From<AnsiColor> for Color {
    fn from(color: AnsiColor) -> Self {
        match color {
            AnsiColor::Named(named) => Self::Named(named.into()),
            AnsiColor::Indexed(index) => Self::Indexed(index),
            AnsiColor::Spec(rgb) => Self::Spec(rgb.into()),
        }
    }
}

impl From<alacritty_terminal::vte::ansi::Rgb> for Rgb {
    fn from(rgb: alacritty_terminal::vte::ansi::Rgb) -> Self {
        Self {
            r: rgb.r,
            g: rgb.g,
            b: rgb.b,
        }
    }
}

// The match is exhaustive on purpose: upgrading the pinned Alacritty makes a
// new named color a compile error here instead of silently mapping it to the
// wrong slot. U-01 reviews the upstream change and updates both sides.
impl From<AnsiNamedColor> for NamedColor {
    fn from(named: AnsiNamedColor) -> Self {
        match named {
            AnsiNamedColor::Black => Self::Black,
            AnsiNamedColor::Red => Self::Red,
            AnsiNamedColor::Green => Self::Green,
            AnsiNamedColor::Yellow => Self::Yellow,
            AnsiNamedColor::Blue => Self::Blue,
            AnsiNamedColor::Magenta => Self::Magenta,
            AnsiNamedColor::Cyan => Self::Cyan,
            AnsiNamedColor::White => Self::White,
            AnsiNamedColor::BrightBlack => Self::BrightBlack,
            AnsiNamedColor::BrightRed => Self::BrightRed,
            AnsiNamedColor::BrightGreen => Self::BrightGreen,
            AnsiNamedColor::BrightYellow => Self::BrightYellow,
            AnsiNamedColor::BrightBlue => Self::BrightBlue,
            AnsiNamedColor::BrightMagenta => Self::BrightMagenta,
            AnsiNamedColor::BrightCyan => Self::BrightCyan,
            AnsiNamedColor::BrightWhite => Self::BrightWhite,
            AnsiNamedColor::Foreground => Self::Foreground,
            AnsiNamedColor::Background => Self::Background,
            AnsiNamedColor::Cursor => Self::Cursor,
            AnsiNamedColor::DimBlack => Self::DimBlack,
            AnsiNamedColor::DimRed => Self::DimRed,
            AnsiNamedColor::DimGreen => Self::DimGreen,
            AnsiNamedColor::DimYellow => Self::DimYellow,
            AnsiNamedColor::DimBlue => Self::DimBlue,
            AnsiNamedColor::DimMagenta => Self::DimMagenta,
            AnsiNamedColor::DimCyan => Self::DimCyan,
            AnsiNamedColor::DimWhite => Self::DimWhite,
            AnsiNamedColor::BrightForeground => Self::BrightForeground,
            AnsiNamedColor::DimForeground => Self::DimForeground,
        }
    }
}
