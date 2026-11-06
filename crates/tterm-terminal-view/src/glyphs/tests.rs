//! Tests for the built-in glyph geometry.
//!
//! The coverage and the bounds are the contract P0-10 relies on: every
//! character Alacritty draws is present at every cell size the view can
//! measure, no shape leaves the cell, and the pieces of a cell meet.

use super::BuiltinGlyph;
use super::CellMetrics;
use super::GlyphFill;
use super::GlyphShape;
use super::Point;
use super::builtin_glyph;

/// Cell sizes the view may measure, including odd and square cells.
const SIZES: [CellMetrics; 5] = [
    CellMetrics::new(8.0, 16.0),
    CellMetrics::new(10.0, 20.0),
    CellMetrics::new(7.0, 14.0),
    CellMetrics::new(24.0, 24.0),
    CellMetrics::new(9.0, 17.0),
];

fn glyph(character: char, metrics: CellMetrics) -> BuiltinGlyph {
    builtin_glyph(character, metrics)
        .unwrap_or_else(|| panic!("no built-in glyph for {character:?} at {metrics:?}"))
}

fn rect_contains(rect: &super::Rect, x: f32, y: f32) -> bool {
    x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
}

/// Ray casting for the filled polygons.
fn polygon_contains(points: &[Point], x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for point in points {
        let crosses = (point.y > y) != (previous.y > y)
            && x < (previous.x - point.x) * (y - point.y) / (previous.y - point.y) + point.x;
        if crosses {
            inside = !inside;
        }
        previous = *point;
    }
    inside
}

fn covers(glyph: &BuiltinGlyph, x: f32, y: f32) -> bool {
    glyph.shapes().iter().any(|shape| match shape {
        GlyphShape::Rect(rect) => rect_contains(rect, x, y),
        GlyphShape::Polygon(points) => polygon_contains(points, x, y),
    })
}

fn assert_inside(glyph: &BuiltinGlyph, metrics: CellMetrics) {
    let epsilon = 0.001;
    for shape in glyph.shapes() {
        match shape {
            GlyphShape::Rect(rect) => {
                assert!(
                    rect.width > 0.0 && rect.height > 0.0,
                    "empty rect in {glyph:?}"
                );
                assert!(rect.x >= -epsilon, "rect left of the cell: {glyph:?}");
                assert!(rect.y >= -epsilon, "rect above the cell: {glyph:?}");
                assert!(
                    rect.x + rect.width <= metrics.width + epsilon,
                    "rect right of the cell: {glyph:?}"
                );
                assert!(
                    rect.y + rect.height <= metrics.height + epsilon,
                    "rect below the cell: {glyph:?}"
                );
            }
            GlyphShape::Polygon(points) => {
                assert!(points.len() >= 3, "polygon with fewer than 3 points");
                for point in points {
                    assert!(point.x >= -epsilon, "point left of the cell: {point:?}");
                    assert!(point.y >= -epsilon, "point above the cell: {point:?}");
                    assert!(
                        point.x <= metrics.width + epsilon,
                        "point right of the cell: {point:?}"
                    );
                    assert!(
                        point.y <= metrics.height + epsilon,
                        "point below the cell: {point:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_supported_character_has_shapes_inside_the_cell() {
    let ranges = [
        0x2500..=0x257f,
        0x2580..=0x259f,
        0x1fb00..=0x1fb3b,
        0x1fb82..=0x1fb8b,
        0xe0b0..=0xe0b3,
    ];
    for range in ranges {
        for code in range {
            let character = char::from_u32(code).unwrap();
            for metrics in SIZES {
                let glyph = glyph(character, metrics);
                assert!(
                    !glyph.shapes().is_empty(),
                    "{character:?} has no shapes at {metrics:?}"
                );
                assert_inside(&glyph, metrics);
            }
        }
    }
}

#[test]
fn unknown_characters_are_left_to_the_font() {
    for character in [
        'a',
        'é',
        '\u{24ff}',
        '\u{25a0}',
        '\u{1fb3c}',
        '\u{1fb81}',
        '\u{1fb8c}',
    ] {
        assert_eq!(builtin_glyph(character, SIZES[0]), None);
    }
    // A cell that has not been measured has no glyph.
    assert_eq!(builtin_glyph('─', CellMetrics::new(0.0, 16.0)), None);
    assert_eq!(builtin_glyph('─', CellMetrics::new(8.0, 0.0)), None);
}

#[test]
fn straight_lines_span_the_whole_cell() {
    for metrics in SIZES {
        let horizontal = glyph('─', metrics);
        for x in [0.0, metrics.width / 2.0, metrics.width] {
            assert!(
                covers(&horizontal, x, metrics.height / 2.0),
                "─ does not cover ({x}, mid) at {metrics:?}"
            );
        }
        let vertical = glyph('│', metrics);
        for y in [0.0, metrics.height / 2.0, metrics.height] {
            assert!(
                covers(&vertical, metrics.width / 2.0, y),
                "│ does not cover (mid, {y}) at {metrics:?}"
            );
        }
        let cross = glyph('┼', metrics);
        for point in [
            (0.0, metrics.height / 2.0),
            (metrics.width, metrics.height / 2.0),
            (metrics.width / 2.0, 0.0),
            (metrics.width / 2.0, metrics.height),
        ] {
            assert!(
                covers(&cross, point.0, point.1),
                "┼ does not reach {point:?} at {metrics:?}"
            );
        }
    }
}

#[test]
fn a_full_block_and_a_partial_block_are_consistent() {
    for metrics in SIZES {
        let full = glyph('█', metrics);
        for point in [
            (0.0, 0.0),
            (metrics.width, 0.0),
            (0.0, metrics.height),
            (metrics.width, metrics.height),
            (metrics.width / 2.0, metrics.height / 2.0),
        ] {
            assert!(
                covers(&full, point.0, point.1),
                "█ does not cover {point:?} at {metrics:?}"
            );
        }
        let top = glyph('▀', metrics);
        assert!(covers(&top, metrics.width / 2.0, 0.5));
        assert!(!covers(&top, metrics.width / 2.0, metrics.height - 0.5));
    }
}

#[test]
fn quadrants_and_sextants_meet_without_gaps() {
    let metrics = CellMetrics::new(12.0, 18.0);
    let quadrants = glyph('▚', metrics);
    assert!(covers(&quadrants, 1.0, 1.0));
    assert!(covers(
        &quadrants,
        metrics.width - 1.0,
        metrics.height - 1.0
    ));
    assert!(!covers(&quadrants, metrics.width - 1.0, 1.0));
    assert!(!covers(&quadrants, 1.0, metrics.height - 1.0));

    // U+1FB3B is every sextant except the top left, so it has to cover the
    // other five and leave exactly that one empty.
    let sextant = glyph('\u{1fb3b}', metrics);
    for point in [
        (metrics.width - 1.0, 1.0),
        (1.0, metrics.height / 2.0),
        (metrics.width - 1.0, metrics.height / 2.0),
        (1.0, metrics.height - 1.0),
        (metrics.width - 1.0, metrics.height - 1.0),
    ] {
        assert!(
            covers(&sextant, point.0, point.1),
            "the sextant does not cover {point:?}"
        );
    }
    assert!(!covers(&sextant, 1.0, 1.0));
    // U+1FB00 is only the top-left sextant.
    let top_left = glyph('\u{1fb00}', metrics);
    assert!(covers(&top_left, 1.0, 1.0));
    assert!(!covers(&top_left, metrics.width - 1.0, 1.0));
}

#[test]
fn shades_use_the_alacritty_coverage_steps() {
    // Alacritty's `builtin_font.rs` calls 64, 128 and 192 the fill alpha
    // steps. A shade enters the atlas as `BitmapBuffer::Rgb` with
    // `multicolor` off (renderer/text/atlas.rs), and `res/glsl3/text.f.glsl`
    // multiplies that mask into the foreground color's alpha, so the number
    // is coverage of the foreground color, not a gray: ░ is 64/255, ▒ is
    // 128/255 and ▓ is 192/255 opacity, and █ is the foreground color itself.
    for (character, coverage) in [('░', 64), ('▒', 128), ('▓', 192)] {
        let shade = glyph(character, SIZES[0]);
        assert_eq!(shade.fill(), GlyphFill::ForegroundAlpha(coverage));
        assert!(covers(&shade, 1.0, 1.0));
    }
    let full = glyph('█', SIZES[0]);
    assert_eq!(full.fill(), GlyphFill::Foreground);
    assert!(covers(&full, 1.0, 1.0));
    // Everything else uses the foreground color too.
    assert_eq!(glyph('─', SIZES[0]).fill(), GlyphFill::Foreground);
}

#[test]
fn powerline_is_mirrored_for_the_right_hand_pair() {
    for metrics in SIZES {
        let left = glyph('\u{e0b0}', metrics);
        let right = glyph('\u{e0b2}', metrics);
        // The left triangle has its full-height edge on the left, the right
        // one on the right; every other point is the mirror image.
        assert!(covers(&left, 0.0, metrics.height / 2.0));
        assert!(covers(&right, metrics.width, metrics.height / 2.0));
        for (x, y) in [
            (0.0, 1.0),
            (metrics.width / 2.0, metrics.height / 2.0),
            (metrics.width - 1.0, metrics.height / 3.0),
        ] {
            assert_eq!(
                covers(&left, x, y),
                covers(&right, metrics.width - x, y),
                "the right triangle is not the mirror of the left at ({x}, {y})"
            );
        }
    }
}
