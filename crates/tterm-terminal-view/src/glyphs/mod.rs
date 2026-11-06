// Derived from Alacritty's `alacritty/src/renderer/text/builtin_font.rs`,
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the rasterized bitmap is replaced by cell-local rectangles and
// polygons, which P0-10 paints with GPUI at any cell size. The character
// coverage, stroke sizes and block fractions are Alacritty's. The source is
// split into `geometry.rs`, `lines.rs`, `blocks.rs` and `powerline.rs` to
// stay under the repository's file limit.

//! Built-in glyph geometry for characters that must cover their whole cell.
//!
//! The font a terminal uses often lacks box drawing, block elements or
//! Powerline symbols, or draws them with gaps. Alacritty replaces those
//! characters with its own shapes; this module keeps the same coverage and
//! emits geometry instead of a bitmap:
//!
//! - [`GlyphShape::Rect`] for the straight arms, blocks and quadrants;
//! - [`GlyphShape::Polygon`] for diagonals, rounded corners, Powerline
//!   triangles and the shade fills.
//!
//! Coordinates are cell-local device pixels with the origin at the cell's
//! top-left corner, so the view adds the cell origin and passes the shapes
//! to `Window::paint_quad` or a fill-only `PathBuilder`. [`GlyphFill`] says
//! whether the shapes use the cell's foreground color or one of Alacritty's
//! fixed shade grays.

pub mod blocks;
pub mod geometry;
pub mod lines;
pub mod powerline;

use geometry::Geometry;

/// The size of one cell in device pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CellMetrics {
    /// Width of the cell in pixels.
    pub width: f32,
    /// Height of the cell in pixels.
    pub height: f32,
}

impl CellMetrics {
    /// Creates metrics for one cell.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// A point in cell-local pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Distance from the cell's left edge.
    pub x: f32,
    /// Distance from the cell's top edge.
    pub y: f32,
}

impl Point {
    /// Creates a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// An axis-aligned rectangle in cell-local pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width; always positive.
    pub width: f32,
    /// Height; always positive.
    pub height: f32,
}

/// One filled piece of a built-in glyph.
#[derive(Clone, Debug, PartialEq)]
pub enum GlyphShape {
    /// An axis-aligned rectangle.
    Rect(Rect),
    /// A filled polygon, in order; the path closes itself.
    Polygon(Vec<Point>),
}

/// The color a built-in glyph is filled with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphFill {
    /// Draw with the cell's foreground color.
    Foreground,
    /// Draw with Alacritty's fixed gray for a shade, from 0 to 255.
    ///
    /// The values are 64 for `░`, 128 for `▒`, 192 for `▓` and 255 for `█`.
    Gray(u8),
}

/// A built-in glyph in cell-local pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinGlyph {
    shapes: Vec<GlyphShape>,
    fill: GlyphFill,
}

impl BuiltinGlyph {
    pub(crate) fn new(shapes: Vec<GlyphShape>, fill: GlyphFill) -> Self {
        Self { shapes, fill }
    }

    /// The filled pieces, in painting order.
    pub fn shapes(&self) -> &[GlyphShape] {
        &self.shapes
    }

    /// The color to paint the shapes with.
    pub fn fill(&self) -> GlyphFill {
        self.fill
    }
}

/// The built-in glyph for `character`, or `None` when the character is
/// drawn by the font.
///
/// The ranges are Alacritty's: box drawing U+2500..U+257F, block elements
/// U+2580..U+259F, Powerline U+E0B0..U+E0B3, and the legacy computing
/// sextants and blocks U+1FB00..U+1FB3B and U+1FB82..U+1FB8B.
pub fn builtin_glyph(character: char, metrics: CellMetrics) -> Option<BuiltinGlyph> {
    if !metrics.width.is_finite()
        || !metrics.height.is_finite()
        || metrics.width <= 0.0
        || metrics.height <= 0.0
    {
        return None;
    }
    let glyph = match character {
        '\u{2500}'..='\u{257f}' => lines::box_drawing(character, metrics)?,
        '\u{2580}'..='\u{259f}' | '\u{1fb00}'..='\u{1fb3b}' | '\u{1fb82}'..='\u{1fb8b}' => {
            blocks::block_element(character, metrics)?
        }
        '\u{e0b0}'..='\u{e0b3}' => powerline::powerline(character, metrics)?,
        _ => return None,
    };
    Some(glyph)
}

/// Builds a glyph with the foreground fill.
pub(crate) fn foreground(geometry: Geometry) -> BuiltinGlyph {
    BuiltinGlyph::new(geometry.finish(), GlyphFill::Foreground)
}

#[cfg(test)]
mod tests;
