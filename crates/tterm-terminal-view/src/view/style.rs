//! Resolving a cell's semantic colors and attributes for one frame.
//!
//! Snapshot cells keep the colors the program asked for. This module applies
//! the current palette, dim, inverse and the bold-to-bright rule once per
//! cell while a row is planned; the result is cached with the row.

use gpui_kit::Hsla;
use tterm_terminal::cell::{Cell, Underline};
use tterm_terminal::palette::{Rgba, TerminalPalette};

/// The foreground and font attributes of a text run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// The resolved foreground color.
    pub foreground: Rgba,
    /// Bold strokes.
    pub bold: bool,
    /// Italic strokes.
    pub italic: bool,
    /// Invisible text; it still paints its background.
    pub hidden: bool,
    /// The underline decoration.
    pub underline: Underline,
    /// The underline color, which may differ from the foreground.
    pub underline_color: Rgba,
    /// Struck-through text.
    pub strikeout: bool,
}

/// A cell after palette resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResolvedStyle {
    /// What the glyph is painted with.
    pub text: TextStyle,
    /// What the cell's background is painted with.
    pub background: Rgba,
}

impl ResolvedStyle {
    /// Resolves one snapshot cell against `palette`.
    ///
    /// The order matches xterm.js: bold uses the bright variant of the eight
    /// standard colors, dim blends toward the background, then inverse swaps
    /// the two. A true color is never adjusted.
    pub fn for_cell(cell: &Cell, palette: &TerminalPalette) -> Self {
        let mut foreground = palette.resolve(cell.foreground, cell.attributes.bold);
        let mut background = palette.resolve(cell.background, false);
        if cell.attributes.dim {
            foreground = foreground.mix(background, 0.5);
        }
        if cell.attributes.inverse {
            std::mem::swap(&mut foreground, &mut background);
        }
        let underline_color = cell
            .underline_color
            .map(|color| palette.resolve(color, false))
            .unwrap_or(foreground);
        Self {
            text: TextStyle {
                foreground,
                bold: cell.attributes.bold,
                italic: cell.attributes.italic,
                hidden: cell.attributes.hidden,
                underline: cell.attributes.underline,
                underline_color,
                strikeout: cell.attributes.strikeout,
            },
            background,
        }
    }

    /// Whether the cell uses the default background and needs no rectangle.
    pub fn has_background(&self, palette: &TerminalPalette) -> bool {
        self.background != palette.background && self.background.a > 0
    }
}

/// Converts a palette color to the color type GPUI paints with.
pub fn hsla(color: Rgba) -> Hsla {
    Hsla::from(gpui_kit::Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: f32::from(color.a) / 255.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_terminal::cell::Attributes;
    use tterm_terminal::color::{Color, NamedColor};

    fn cell() -> Cell {
        Cell {
            ch: 'a',
            foreground: Color::Named(NamedColor::Foreground),
            background: Color::Named(NamedColor::Background),
            attributes: Attributes::default(),
            underline_color: None,
            hyperlink: None,
            zerowidth: None,
        }
    }

    #[test]
    fn default_cells_use_the_palette_defaults() {
        let palette = TerminalPalette::default();
        let resolved = ResolvedStyle::for_cell(&cell(), &palette);
        assert_eq!(resolved.text.foreground, palette.foreground);
        assert_eq!(resolved.background, palette.background);
        assert!(!resolved.has_background(&palette));
    }

    #[test]
    fn inverse_swaps_the_two_colors() {
        let palette = TerminalPalette::default();
        let mut cell = cell();
        cell.attributes.inverse = true;
        let resolved = ResolvedStyle::for_cell(&cell, &palette);
        assert_eq!(resolved.text.foreground, palette.background);
        assert_eq!(resolved.background, palette.foreground);
        assert!(resolved.has_background(&palette));
    }

    #[test]
    fn dim_blends_toward_the_background() {
        let palette = TerminalPalette::default();
        let mut cell = cell();
        cell.attributes.dim = true;
        let resolved = ResolvedStyle::for_cell(&cell, &palette);
        assert_eq!(
            resolved.text.foreground,
            palette.foreground.mix(palette.background, 0.5)
        );
    }

    #[test]
    fn bold_red_uses_the_bright_color() {
        let palette = TerminalPalette::default();
        let mut cell = cell();
        cell.foreground = Color::Named(NamedColor::Red);
        cell.attributes.bold = true;
        let resolved = ResolvedStyle::for_cell(&cell, &palette);
        assert_eq!(resolved.text.foreground, palette.bright_red);
        assert!(resolved.text.bold);
    }

    #[test]
    fn the_underline_color_defaults_to_the_foreground() {
        let palette = TerminalPalette::default();
        let mut cell = cell();
        cell.attributes.underline = Underline::Curly;
        let resolved = ResolvedStyle::for_cell(&cell, &palette);
        assert_eq!(resolved.text.underline, Underline::Curly);
        assert_eq!(resolved.text.underline_color, palette.foreground);

        cell.underline_color = Some(Color::Named(NamedColor::Blue));
        let resolved = ResolvedStyle::for_cell(&cell, &palette);
        assert_eq!(resolved.text.underline_color, palette.blue);
    }
}
