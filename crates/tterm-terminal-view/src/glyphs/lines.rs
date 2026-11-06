// Derived from Alacritty's `alacritty/src/renderer/text/builtin_font.rs`,
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the pixel canvas becomes rectangles and polygons; the arm table,
// stroke sizes, double lines, dashed lines and arc placement are Alacritty's.
// The rounded corners use a smaller radius than Alacritty's rasterized arc so
// they leave room for the arms at any cell size; this is recorded in the
// P0-28 report.

//! Box drawing characters, U+2500 through U+257F.
//!
//! The straight arms are described by a generated table of four directions,
//! each none, light or heavy. The table is Alacritty's four match blocks,
//! flattened; the dashed, double, rounded and diagonal families are drawn
//! separately because their geometry is not four independent arms.

use std::f32::consts::FRAC_PI_2;
use std::f32::consts::PI;

use super::BuiltinGlyph;
use super::CellMetrics;
use super::foreground;
use super::geometry::Geometry;
use super::geometry::arc_polygon;
use super::geometry::line_polygon;
use super::geometry::stroke_size;

/// No arm in this direction.
const NONE: u8 = 0;
/// A heavy arm, two strokes wide.
const HEAVY: u8 = 2;

/// The built-in drawing of one box drawing character.
pub fn box_drawing(character: char, metrics: CellMetrics) -> Option<BuiltinGlyph> {
    let width = metrics.width;
    let height = metrics.height;
    let stroke = stroke_size(width);

    if let Some((gaps, heavy)) = dashed_horizontal(character) {
        return Some(foreground(draw_dashed_horizontal(
            width, height, stroke, gaps, heavy,
        )));
    }
    if let Some((gaps, heavy)) = dashed_vertical(character) {
        return Some(foreground(draw_dashed_vertical(
            width, height, stroke, gaps, heavy,
        )));
    }
    if let Some(arms) = arms(character) {
        return Some(foreground(draw_arms(width, height, stroke, arms)));
    }
    match character {
        '\u{2550}'..='\u{256c}' => Some(foreground(draw_double(character, width, height, stroke))),
        '\u{256d}'..='\u{2570}' => Some(foreground(draw_rounded(character, width, height, stroke))),
        '\u{2571}'..='\u{2573}' => {
            Some(foreground(draw_diagonal(character, width, height, stroke)))
        }
        _ => None,
    }
}

/// The four arm weights: left, right, top, bottom.
fn arms(character: char) -> Option<[u8; 4]> {
    let entry = BOX_ARMS.iter().find(|entry| entry.0 == character)?;
    Some([entry.1, entry.2, entry.3, entry.4])
}

/// Draws four independent straight arms from the center of the cell.
fn draw_arms(width: f32, height: f32, stroke: f32, arms: [u8; 4]) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let center_x = geometry.center_x();
    let center_y = geometry.center_y();
    let [left, right, top, bottom] = arms;
    if left != NONE {
        geometry.h_line(0.0, center_y, center_x, arm_stroke(stroke, left));
    }
    if right != NONE {
        geometry.h_line(
            center_x,
            center_y,
            width - center_x,
            arm_stroke(stroke, right),
        );
    }
    if top != NONE {
        geometry.v_line(center_x, 0.0, center_y, arm_stroke(stroke, top));
    }
    if bottom != NONE {
        geometry.v_line(
            center_x,
            center_y,
            height - center_y,
            arm_stroke(stroke, bottom),
        );
    }
    geometry
}

/// The thickness of one arm.
fn arm_stroke(stroke: f32, arm: u8) -> f32 {
    if arm == HEAVY { stroke * 2.0 } else { stroke }
}

/// The gap count and weight of a horizontal dashed line.
fn dashed_horizontal(character: char) -> Option<(u8, bool)> {
    match character {
        '\u{2504}' => Some((2, false)),
        '\u{2505}' => Some((2, true)),
        '\u{2508}' => Some((3, false)),
        '\u{2509}' => Some((3, true)),
        '\u{254c}' => Some((1, false)),
        '\u{254d}' => Some((1, true)),
        _ => None,
    }
}

/// The gap count and weight of a vertical dashed line.
fn dashed_vertical(character: char) -> Option<(u8, bool)> {
    match character {
        '\u{2506}' => Some((2, false)),
        '\u{2507}' => Some((2, true)),
        '\u{250a}' => Some((3, false)),
        '\u{250b}' => Some((3, true)),
        '\u{254e}' => Some((1, false)),
        '\u{254f}' => Some((1, true)),
        _ => None,
    }
}

/// Dashes along the horizontal center line.
fn draw_dashed_horizontal(width: f32, height: f32, stroke: f32, gaps: u8, heavy: bool) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let thickness = if heavy { stroke * 2.0 } else { stroke };
    let gap = (width / 8.0).round().max(1.0);
    let dash = ((width - gap * f32::from(gaps)) / (f32::from(gaps) + 1.0)).max(1.0);
    let center_y = geometry.center_y();
    for index in 0..=gaps {
        let x = (f32::from(index) * (dash + gap)).min(width);
        geometry.h_line(x, center_y, dash, thickness);
    }
    geometry
}

/// Dashes along the vertical center line.
fn draw_dashed_vertical(width: f32, height: f32, stroke: f32, gaps: u8, heavy: bool) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let thickness = if heavy { stroke * 2.0 } else { stroke };
    let gap = (height / 8.0).round().max(1.0);
    let dash = ((height - gap * f32::from(gaps)) / (f32::from(gaps) + 1.0)).max(1.0);
    let center_x = geometry.center_x();
    for index in 0..=gaps {
        let y = (f32::from(index) * (dash + gap)).min(height);
        geometry.v_line(center_x, y, dash, thickness);
    }
    geometry
}

/// Draws one of the light-and-double line characters.
///
/// This is Alacritty's `\u{2550}..=\u{256c}` branch: find the two vertical
/// and two horizontal line positions, then draw the left, right, top and
/// bottom parts with the sizes Alacritty's match arms give.
fn draw_double(character: char, width: f32, height: f32, stroke: f32) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let center_x = geometry.center_x();
    let center_y = geometry.center_y();
    let vertical_bounds = super::geometry::v_line_bounds(center_x, stroke, width);
    let horizontal_bounds = super::geometry::h_line_bounds(center_y, stroke, height);

    let v_lines = match character {
        '\u{2552}' | '\u{2555}' | '\u{2558}' | '\u{255b}' | '\u{255e}' | '\u{2561}'
        | '\u{2564}' | '\u{2567}' | '\u{256a}' => (center_x, center_x),
        _ => (
            (vertical_bounds.0 - 1.0).max(0.0),
            (vertical_bounds.1 + 1.0).min(width),
        ),
    };
    let h_lines = match character {
        '\u{2553}' | '\u{2556}' | '\u{2559}' | '\u{255c}' | '\u{255f}' | '\u{2562}'
        | '\u{2565}' | '\u{2568}' | '\u{256b}' => (center_y, center_y),
        _ => (
            (horizontal_bounds.0 - 1.0).max(0.0),
            (horizontal_bounds.1 + 1.0).min(height),
        ),
    };

    let v_left = super::geometry::v_line_bounds(v_lines.0, stroke, width);
    let v_right = super::geometry::v_line_bounds(v_lines.1, stroke, width);
    let h_top = super::geometry::h_line_bounds(h_lines.0, stroke, height);
    let h_bottom = super::geometry::h_line_bounds(h_lines.1, stroke, height);

    let (top_left, bottom_left) = match character {
        '\u{2550}' | '\u{256b}' => (center_x, center_x),
        '\u{2555}'..='\u{2557}' => (v_right.1, v_left.1),
        '\u{255b}'..='\u{255d}' => (v_left.1, v_right.1),
        '\u{2561}'..='\u{2563}' | '\u{256a}' | '\u{256c}' => (v_left.1, v_left.1),
        '\u{2564}'..='\u{2568}' => (center_x, v_left.1),
        '\u{2569}' => (v_left.1, center_x),
        _ => (0.0, 0.0),
    };
    let (top_right_x, bottom_right_x, right_size) = match character {
        '\u{2550}' | '\u{2565}' | '\u{256b}' => (center_x, center_x, width),
        '\u{2552}'..='\u{2554}' | '\u{2568}' => (v_left.0, v_right.0, width),
        '\u{2558}'..='\u{255a}' => (v_right.0, v_left.0, width),
        '\u{255e}'..='\u{2560}' | '\u{256a}' | '\u{256c}' => (v_right.0, v_right.0, width),
        '\u{2564}' | '\u{2566}' => (center_x, v_right.0, width),
        '\u{2567}' | '\u{2569}' => (v_right.0, center_x, width),
        _ => (0.0, 0.0, 0.0),
    };
    let (left_top, right_top) = match character {
        '\u{2551}' | '\u{256a}' => (center_y, center_y),
        '\u{2558}'..='\u{255c}' | '\u{2568}' => (h_bottom.1, h_top.1),
        '\u{255d}' => (h_top.1, h_bottom.1),
        '\u{255e}'..='\u{2560}' => (center_y, h_top.1),
        '\u{2561}'..='\u{2563}' => (h_top.1, center_y),
        '\u{2567}' | '\u{2569}' | '\u{256b}' | '\u{256c}' => (h_top.1, h_top.1),
        _ => (0.0, 0.0),
    };
    let (bottom_left_y, bottom_right_y, bottom_size) = match character {
        '\u{2551}' | '\u{256a}' => (center_y, center_y, height),
        '\u{2552}'..='\u{2554}' => (h_top.0, h_bottom.0, height),
        '\u{2555}'..='\u{2557}' => (h_bottom.0, h_top.0, height),
        '\u{255e}'..='\u{2560}' => (center_y, h_bottom.0, height),
        '\u{2561}'..='\u{2563}' => (h_bottom.0, center_y, height),
        '\u{2564}'..='\u{2566}' | '\u{256b}' | '\u{256c}' => (h_bottom.0, h_bottom.0, height),
        _ => (0.0, 0.0, 0.0),
    };

    geometry.h_line(0.0, h_lines.0, top_left, stroke);
    geometry.h_line(0.0, h_lines.1, bottom_left, stroke);
    geometry.h_line(top_right_x, h_lines.0, right_size, stroke);
    geometry.h_line(bottom_right_x, h_lines.1, right_size, stroke);
    geometry.v_line(v_lines.0, 0.0, left_top, stroke);
    geometry.v_line(v_lines.1, 0.0, right_top, stroke);
    geometry.v_line(v_lines.0, bottom_left_y, bottom_size, stroke);
    geometry.v_line(v_lines.1, bottom_right_y, bottom_size, stroke);
    geometry
}

/// Draws a rounded corner.
///
/// Alacritty rasterizes a quarter circle whose radius is half the short side.
/// A vector arc of that size would leave no straight arms in a tall cell, so
/// the radius here is half of half the short side; the arms still meet the
/// arc and the shape stays inside the cell.
fn draw_rounded(character: char, width: f32, height: f32, stroke: f32) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let center_x = geometry.center_x();
    let center_y = geometry.center_y();
    let radius = (width.min(height) / 4.0).max(stroke / 2.0);
    let half_pi = FRAC_PI_2;
    let (arc_center, start, end, horizontal, vertical) = match character {
        '\u{256d}' => (
            super::Point::new(center_x + radius, center_y + radius),
            PI,
            PI + half_pi,
            (center_x + radius, width),
            (center_y + radius, height),
        ),
        '\u{256e}' => (
            super::Point::new(center_x - radius, center_y + radius),
            PI + half_pi,
            2.0 * PI,
            (0.0, center_x - radius),
            (center_y + radius, height),
        ),
        '\u{256f}' => (
            super::Point::new(center_x - radius, center_y - radius),
            0.0,
            half_pi,
            (0.0, center_x - radius),
            (0.0, center_y - radius),
        ),
        '\u{2570}' => (
            super::Point::new(center_x + radius, center_y - radius),
            half_pi,
            PI,
            (center_x + radius, width),
            (0.0, center_y - radius),
        ),
        _ => return geometry,
    };
    geometry.polygon(arc_polygon(arc_center, radius, start, end, stroke));
    geometry.h_line(
        horizontal.0,
        center_y,
        (horizontal.1 - horizontal.0).max(0.0),
        stroke,
    );
    geometry.v_line(
        center_x,
        vertical.0,
        (vertical.1 - vertical.0).max(0.0),
        stroke,
    );
    geometry
}

/// Draws the diagonals `╱`, `╲` and `╳`.
fn draw_diagonal(character: char, width: f32, height: f32, stroke: f32) -> Geometry {
    let mut geometry = Geometry::new(width, height);
    let thickness = stroke * 2.0;
    if character == '\u{2571}' || character == '\u{2573}' {
        geometry.polygon(line_polygon(
            super::Point::new(width, 0.0),
            super::Point::new(0.0, height),
            thickness,
        ));
    }
    if character == '\u{2572}' || character == '\u{2573}' {
        geometry.polygon(line_polygon(
            super::Point::new(0.0, 0.0),
            super::Point::new(width, height),
            thickness,
        ));
    }
    geometry
}

/// The four arm weights of every straight box drawing character.
///
/// Generated from Alacritty's four match blocks in the `\u{2500}..` branch:
/// the value is `NONE`, `LIGHT` or `HEAVY` for left, right, top and bottom.
const BOX_ARMS: [(char, u8, u8, u8, u8); 80] = [
    ('\u{2500}', 1, 1, 0, 0),
    ('\u{2501}', 2, 2, 0, 0),
    ('\u{2502}', 0, 0, 1, 1),
    ('\u{2503}', 0, 0, 2, 2),
    ('\u{250c}', 0, 1, 0, 1),
    ('\u{250d}', 0, 2, 0, 1),
    ('\u{250e}', 0, 1, 0, 2),
    ('\u{250f}', 0, 2, 0, 2),
    ('\u{2510}', 1, 0, 0, 1),
    ('\u{2511}', 2, 0, 0, 1),
    ('\u{2512}', 1, 0, 0, 2),
    ('\u{2513}', 2, 0, 0, 2),
    ('\u{2514}', 0, 1, 1, 0),
    ('\u{2515}', 0, 2, 1, 0),
    ('\u{2516}', 0, 1, 2, 0),
    ('\u{2517}', 0, 2, 2, 0),
    ('\u{2518}', 1, 0, 1, 0),
    ('\u{2519}', 2, 0, 1, 0),
    ('\u{251a}', 1, 0, 2, 0),
    ('\u{251b}', 2, 0, 2, 0),
    ('\u{251c}', 0, 1, 1, 1),
    ('\u{251d}', 0, 2, 1, 1),
    ('\u{251e}', 0, 1, 2, 1),
    ('\u{251f}', 0, 1, 1, 2),
    ('\u{2520}', 0, 1, 2, 2),
    ('\u{2521}', 0, 2, 2, 1),
    ('\u{2522}', 0, 2, 1, 2),
    ('\u{2523}', 0, 2, 2, 2),
    ('\u{2524}', 1, 0, 1, 1),
    ('\u{2525}', 2, 0, 1, 1),
    ('\u{2526}', 1, 0, 2, 1),
    ('\u{2527}', 1, 0, 1, 2),
    ('\u{2528}', 1, 0, 2, 2),
    ('\u{2529}', 2, 0, 2, 1),
    ('\u{252a}', 2, 0, 1, 2),
    ('\u{252b}', 2, 0, 2, 2),
    ('\u{252c}', 1, 1, 0, 1),
    ('\u{252d}', 2, 1, 0, 1),
    ('\u{252e}', 1, 2, 0, 1),
    ('\u{252f}', 2, 2, 0, 1),
    ('\u{2530}', 1, 1, 0, 2),
    ('\u{2531}', 2, 1, 0, 2),
    ('\u{2532}', 1, 2, 0, 2),
    ('\u{2533}', 2, 2, 0, 2),
    ('\u{2534}', 1, 1, 1, 0),
    ('\u{2535}', 2, 1, 1, 0),
    ('\u{2536}', 1, 2, 1, 0),
    ('\u{2537}', 2, 2, 1, 0),
    ('\u{2538}', 1, 1, 2, 0),
    ('\u{2539}', 2, 1, 2, 0),
    ('\u{253a}', 1, 2, 2, 0),
    ('\u{253b}', 2, 2, 2, 0),
    ('\u{253c}', 1, 1, 1, 1),
    ('\u{253d}', 2, 1, 1, 1),
    ('\u{253e}', 1, 2, 1, 1),
    ('\u{253f}', 2, 2, 1, 1),
    ('\u{2540}', 1, 1, 2, 1),
    ('\u{2541}', 1, 1, 1, 2),
    ('\u{2542}', 1, 1, 2, 2),
    ('\u{2543}', 2, 1, 2, 1),
    ('\u{2544}', 1, 2, 2, 1),
    ('\u{2545}', 2, 1, 1, 2),
    ('\u{2546}', 1, 2, 1, 2),
    ('\u{2547}', 2, 2, 2, 1),
    ('\u{2548}', 2, 2, 1, 2),
    ('\u{2549}', 2, 1, 2, 2),
    ('\u{254a}', 1, 2, 2, 2),
    ('\u{254b}', 2, 2, 2, 2),
    ('\u{2574}', 1, 0, 0, 0),
    ('\u{2575}', 0, 0, 1, 0),
    ('\u{2576}', 0, 1, 0, 0),
    ('\u{2577}', 0, 0, 0, 1),
    ('\u{2578}', 2, 0, 0, 0),
    ('\u{2579}', 0, 0, 2, 0),
    ('\u{257a}', 0, 2, 0, 0),
    ('\u{257b}', 0, 0, 0, 2),
    ('\u{257c}', 1, 2, 0, 0),
    ('\u{257d}', 0, 0, 1, 2),
    ('\u{257e}', 2, 1, 0, 0),
    ('\u{257f}', 0, 0, 2, 1),
];
