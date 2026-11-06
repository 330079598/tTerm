// Derived from Alacritty's `alacritty/src/renderer/text/builtin_font.rs`,
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the pixel canvas becomes rectangles; the block fractions,
// quadrant and sextant masks and shade coverages are Alacritty's.

//! Block elements, U+2580 through U+259F, and the legacy computing blocks
//! U+1FB00 through U+1FB3B and U+1FB82 through U+1FB8B.
//!
//! The fractions are Alacritty's eighths, rounded to whole pixels so the
//! pieces of one cell always meet. The sextant table is generated from
//! Alacritty's six match blocks: one bit per sixth of the cell.

use super::BuiltinGlyph;
use super::CellMetrics;
use super::GlyphFill;
use super::geometry::Geometry;

/// The built-in drawing of one block element.
pub fn block_element(character: char, metrics: CellMetrics) -> Option<BuiltinGlyph> {
    match character {
        '\u{2580}'..='\u{2587}'
        | '\u{2589}'..='\u{2590}'
        | '\u{2594}'
        | '\u{2595}'
        | '\u{1fb82}'..='\u{1fb8b}' => Some(fraction_block(character, metrics)),
        '\u{2588}' | '\u{2591}'..='\u{2593}' => Some(shade(character, metrics)),
        '\u{2596}'..='\u{259f}' => Some(quadrant(character, metrics)),
        '\u{1fb00}'..='\u{1fb3b}' => Some(sextant(character, metrics)),
        _ => None,
    }
}

/// Draws an eighth-fraction block.
fn fraction_block(character: char, metrics: CellMetrics) -> BuiltinGlyph {
    let width = metrics.width;
    let height = metrics.height;
    let mut geometry = Geometry::new(width, height);

    let rect_width = match character {
        '\u{2589}' | '\u{1fb8b}' => width * 7.0 / 8.0,
        '\u{258a}' | '\u{1fb8a}' => width * 6.0 / 8.0,
        '\u{258b}' | '\u{1fb89}' => width * 5.0 / 8.0,
        '\u{258c}' => width * 4.0 / 8.0,
        '\u{258d}' | '\u{1fb88}' => width * 3.0 / 8.0,
        '\u{258e}' | '\u{1fb87}' => width * 2.0 / 8.0,
        '\u{258f}' => width / 8.0,
        '\u{2590}' => width / 2.0,
        '\u{2595}' => width / 8.0,
        _ => width,
    };
    let (rect_height, from_bottom) = match character {
        '\u{2580}' => (height * 4.0 / 8.0, false),
        '\u{2581}' => (height / 8.0, true),
        '\u{2582}' => (height * 2.0 / 8.0, true),
        '\u{2583}' => (height * 3.0 / 8.0, true),
        '\u{2584}' => (height * 4.0 / 8.0, true),
        '\u{2585}' => (height * 5.0 / 8.0, true),
        '\u{2586}' => (height * 6.0 / 8.0, true),
        '\u{2587}' => (height * 7.0 / 8.0, true),
        '\u{2594}' => (height / 8.0, false),
        '\u{1fb82}' => (height * 2.0 / 8.0, false),
        '\u{1fb83}' => (height * 3.0 / 8.0, false),
        '\u{1fb84}' => (height * 5.0 / 8.0, false),
        '\u{1fb85}' => (height * 6.0 / 8.0, false),
        '\u{1fb86}' => (height * 7.0 / 8.0, false),
        _ => (height, true),
    };
    let y = if from_bottom {
        height - rect_height
    } else {
        0.0
    };
    let x = match character {
        '\u{2590}' => width / 2.0,
        '\u{2595}' | '\u{1fb87}'..='\u{1fb8b}' => width - rect_width,
        _ => 0.0,
    };
    geometry.rect(
        x,
        y.round(),
        rect_width.round().max(1.0),
        rect_height.round().max(1.0),
    );
    super::foreground(geometry)
}

/// Fills the cell with one of the four shade coverages.
fn shade(character: char, metrics: CellMetrics) -> BuiltinGlyph {
    let mut geometry = Geometry::new(metrics.width, metrics.height);
    geometry.rect(0.0, 0.0, metrics.width, metrics.height);
    // Alacritty's 64, 128 and 192 are coverage steps, so each shade is the
    // foreground color at that share of its alpha; █ covers the cell and is
    // the plain foreground color.
    let fill = match character {
        '\u{2591}' => GlyphFill::ForegroundAlpha(64),
        '\u{2592}' => GlyphFill::ForegroundAlpha(128),
        '\u{2593}' => GlyphFill::ForegroundAlpha(192),
        _ => GlyphFill::Foreground,
    };
    BuiltinGlyph::new(geometry.finish(), fill)
}

/// Draws the quadrants U+2596 through U+259F.
fn quadrant(character: char, metrics: CellMetrics) -> BuiltinGlyph {
    let width = metrics.width;
    let height = metrics.height;
    let mut geometry = Geometry::new(width, height);
    let x_center = (width / 2.0).round().max(1.0);
    let y_center = (height / 2.0).round().max(1.0);
    let right = width - x_center;
    let bottom = height - y_center;

    let second = matches!(
        character,
        '\u{2598}' | '\u{2599}' | '\u{259a}' | '\u{259b}' | '\u{259c}'
    );
    let first = matches!(
        character,
        '\u{259b}' | '\u{259c}' | '\u{259d}' | '\u{259e}' | '\u{259f}'
    );
    let third = matches!(
        character,
        '\u{2596}' | '\u{2599}' | '\u{259b}' | '\u{259e}' | '\u{259f}'
    );
    let fourth = matches!(
        character,
        '\u{2597}' | '\u{2599}' | '\u{259a}' | '\u{259c}' | '\u{259f}'
    );

    if second {
        geometry.rect(0.0, 0.0, x_center, y_center);
    }
    if first {
        geometry.rect(x_center, 0.0, right, y_center);
    }
    if third {
        geometry.rect(0.0, y_center, x_center, bottom);
    }
    if fourth {
        geometry.rect(x_center, y_center, right, bottom);
    }
    super::foreground(geometry)
}

/// Draws the sextants U+1FB00 through U+1FB3B.
fn sextant(character: char, metrics: CellMetrics) -> BuiltinGlyph {
    let width = metrics.width;
    let height = metrics.height;
    let mut geometry = Geometry::new(width, height);
    let x_center = (width / 2.0).round().max(1.0);
    let row = (height / 3.0).round().max(1.0);
    let last_row = (height - 2.0 * row).max(0.0);
    let right = width - x_center;

    let Some(mask) = SEXTANTS
        .iter()
        .find(|entry| entry.0 == character)
        .map(|entry| entry.1)
    else {
        return super::foreground(geometry);
    };
    let pieces = [
        (0.0, 0.0, x_center, row),
        (x_center, 0.0, right, row),
        (0.0, row, x_center, row),
        (x_center, row, right, row),
        (0.0, 2.0 * row, x_center, last_row),
        (x_center, 2.0 * row, right, last_row),
    ];
    for (index, piece) in pieces.iter().enumerate() {
        if mask & (1 << index) != 0 {
            geometry.rect(piece.0, piece.1, piece.2, piece.3);
        }
    }
    super::foreground(geometry)
}

/// The six-bit masks of the sextant characters.
///
/// Bit 0 is top left, then top right, middle left, middle right, bottom left
/// and bottom right. Generated from Alacritty's six match blocks.
const SEXTANTS: [(char, u8); 60] = [
    ('\u{1fb00}', 0b000001),
    ('\u{1fb01}', 0b000010),
    ('\u{1fb02}', 0b000011),
    ('\u{1fb03}', 0b000100),
    ('\u{1fb04}', 0b000101),
    ('\u{1fb05}', 0b000110),
    ('\u{1fb06}', 0b000111),
    ('\u{1fb07}', 0b001000),
    ('\u{1fb08}', 0b001001),
    ('\u{1fb09}', 0b001010),
    ('\u{1fb0a}', 0b001011),
    ('\u{1fb0b}', 0b001100),
    ('\u{1fb0c}', 0b001101),
    ('\u{1fb0d}', 0b001110),
    ('\u{1fb0e}', 0b001111),
    ('\u{1fb0f}', 0b010000),
    ('\u{1fb10}', 0b010001),
    ('\u{1fb11}', 0b010010),
    ('\u{1fb12}', 0b010011),
    ('\u{1fb13}', 0b010100),
    ('\u{1fb14}', 0b010110),
    ('\u{1fb15}', 0b010111),
    ('\u{1fb16}', 0b011000),
    ('\u{1fb17}', 0b011001),
    ('\u{1fb18}', 0b011010),
    ('\u{1fb19}', 0b011011),
    ('\u{1fb1a}', 0b011100),
    ('\u{1fb1b}', 0b011101),
    ('\u{1fb1c}', 0b011110),
    ('\u{1fb1d}', 0b011111),
    ('\u{1fb1e}', 0b100000),
    ('\u{1fb1f}', 0b100001),
    ('\u{1fb20}', 0b100010),
    ('\u{1fb21}', 0b100011),
    ('\u{1fb22}', 0b100100),
    ('\u{1fb23}', 0b100101),
    ('\u{1fb24}', 0b100110),
    ('\u{1fb25}', 0b100111),
    ('\u{1fb26}', 0b101000),
    ('\u{1fb27}', 0b101001),
    ('\u{1fb28}', 0b101011),
    ('\u{1fb29}', 0b101100),
    ('\u{1fb2a}', 0b101101),
    ('\u{1fb2b}', 0b101110),
    ('\u{1fb2c}', 0b101111),
    ('\u{1fb2d}', 0b110000),
    ('\u{1fb2e}', 0b110001),
    ('\u{1fb2f}', 0b110010),
    ('\u{1fb30}', 0b110011),
    ('\u{1fb31}', 0b110100),
    ('\u{1fb32}', 0b110101),
    ('\u{1fb33}', 0b110110),
    ('\u{1fb34}', 0b110111),
    ('\u{1fb35}', 0b111000),
    ('\u{1fb36}', 0b111001),
    ('\u{1fb37}', 0b111010),
    ('\u{1fb38}', 0b111011),
    ('\u{1fb39}', 0b111100),
    ('\u{1fb3a}', 0b111101),
    ('\u{1fb3b}', 0b111110),
];
