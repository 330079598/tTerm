// Derived from Alacritty's `alacritty/src/renderer/text/builtin_font.rs`,
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the pixel canvas becomes rectangles; the row loop, slope and
// arrow notch are Alacritty's `powerline_drawing`.

//! The four Powerline symbols, U+E0B0 through U+E0B3.
//!
//! They are drawn from horizontal rows whose width follows a 45-degree
//! slope, as in Alacritty: a filled triangle for U+E0B0/U+E0B2 and a notched
//! arrow for U+E0B1/U+E0B3. The right-hand pair is the left-hand pair
//! mirrored horizontally.

use super::BuiltinGlyph;
use super::CellMetrics;
use super::foreground;
use super::geometry::Geometry;
use super::geometry::stroke_size;

/// The built-in drawing of one Powerline symbol.
pub fn powerline(character: char, metrics: CellMetrics) -> Option<BuiltinGlyph> {
    let width = metrics.width;
    let height = metrics.height;
    let stroke = stroke_size(width);
    let extra_thickness = (stroke - 1.0).max(0.0);
    // Work in whole rows so the one-pixel steps keep their shape, and scale
    // them to the measured height so the glyph fills a fractional cell.
    let rows = height.round().max(1.0);
    let scale = height / rows;
    let top_y = scale;
    let bottom_y = height - 2.0 * scale;
    let x_intersection = (rows + 1.0) / 2.0 - 1.0;
    // Do not use the built-in shape if the cell is so narrow that the tip
    // would be cut off; Alacritty has the same guard.
    if x_intersection - width > 1.0 {
        return None;
    }

    let mut geometry = Geometry::new(width, height);
    let arrow = matches!(character, '\u{e0b1}' | '\u{e0b3}');
    // The row loop is written in whole rows; stretch it to the measured
    // width so the tip reaches the cell edge at any aspect ratio.
    let x_scale = if x_intersection > 0.0 {
        width / x_intersection
    } else {
        1.0
    };
    let mut x = 0.0;
    while x < x_intersection {
        let y_top = x + top_y;
        let y_bottom = bottom_y - x;
        let x0 = x * x_scale;
        let x1 = (x + 1.0) * x_scale;
        if !arrow {
            geometry.rect(0.0, y_top, x1, scale);
            geometry.rect(0.0, y_bottom, x1, scale);
        } else {
            let inner_top = (y_top + extra_thickness).min(y_bottom);
            let inner_bottom = (y_bottom - extra_thickness).max(y_top);
            if x1 >= width - 0.001 {
                // The last strip closes the arrow's point.
                geometry.rect(x0, y_top, width - x0, y_bottom - y_top + scale);
                break;
            }
            geometry.rect(x0, y_top, x_scale, inner_top - y_top + scale);
            geometry.rect(x0, inner_bottom, x_scale, y_bottom - inner_bottom + scale);
        }
        x += 1.0;
    }

    if matches!(character, '\u{e0b2}' | '\u{e0b3}') {
        geometry.flip_horizontal();
    }
    Some(foreground(geometry))
}
