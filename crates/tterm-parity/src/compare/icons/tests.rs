use super::*;
use crate::layout::ViewportSnapshot;
use image::Rgba;

const BACKGROUND: [u8; 4] = [255, 255, 255, 255];
const STROKE: [u8; 4] = [113, 113, 122, 255];

/// A 40x40 physical-pixel screen at DPR 2 with one 8x8 logical icon at (6, 6).
fn layout() -> LayoutDocument {
    LayoutDocument {
        version: crate::layout::SCHEMA_VERSION,
        screen: "s".to_owned(),
        viewport: ViewportSnapshot {
            width: 20,
            height: 20,
            device_scale_factor: 2.0,
        },
        elements: Vec::new(),
        text_regions: Vec::new(),
        icons: vec![IconSnapshot {
            bounds: BoundsSnapshot::new(6.0, 6.0, 8.0, 8.0),
            color: STROKE,
            opacity: 1.0,
        }],
    }
}

/// A vertical stroke three pixels wide starting at column `x`, with optional
/// half-covered pixels on both sides.
fn stroke(x: u32, color: [u8; 4], soft_edges: bool) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(40, 40, Rgba(BACKGROUND));
    let half = |value: u8, back: u8| ((u16::from(value) + u16::from(back)) / 2) as u8;
    for y in 14..26 {
        for column in x..x + 3 {
            image.put_pixel(column, y, Rgba(color));
        }
        if soft_edges {
            let edge = [
                half(color[0], 255),
                half(color[1], 255),
                half(color[2], 255),
                255,
            ];
            image.put_pixel(x - 1, y, Rgba(edge));
            image.put_pixel(x + 3, y, Rgba(edge));
        }
    }
    image
}

fn report(reference: &RgbaImage, actual: &RgbaImage) -> IconReport {
    check_icons(reference, actual, &layout()).remove(0)
}

#[test]
fn identical_icons_pass() {
    let image = stroke(18, STROKE, true);
    let result = report(&image, &image);
    assert!(result.passed, "{result:?}");
    assert_eq!(result.ink_ratio, 1.0);
}

#[test]
fn antialiasing_spread_differently_passes() {
    // Same stroke, but one side antialiases into a wider soft edge with the
    // same total coverage: the case the per-pixel gate cannot accept.
    let reference = stroke(18, STROKE, true);
    let mut actual = stroke(18, STROKE, false);
    let quarter = |value: u8| ((3 * 255 + u16::from(value)) / 4) as u8;
    let soft = [
        quarter(STROKE[0]),
        quarter(STROKE[1]),
        quarter(STROKE[2]),
        255,
    ];
    for y in 14..26 {
        for column in [16, 17, 21, 22] {
            actual.put_pixel(column, y, Rgba(soft));
        }
    }
    let result = report(&reference, &actual);
    assert!(result.passed, "{result:?}");
}

#[test]
fn a_missing_icon_fails_on_ink() {
    let reference = stroke(18, STROKE, true);
    let actual = RgbaImage::from_pixel(40, 40, Rgba(BACKGROUND));
    let result = report(&reference, &actual);
    assert!(!result.passed);
    assert_eq!(result.ink_ratio, 0.0);
}

#[test]
fn a_moved_icon_fails_on_the_centroid() {
    // One logical pixel is two physical pixels at DPR 2.
    let result = report(&stroke(18, STROKE, true), &stroke(20, STROKE, true));
    assert!(!result.passed);
    assert!((result.centroid_delta - 1.0).abs() < 0.01, "{result:?}");
}

#[test]
fn a_recoloured_icon_fails_on_the_stroke_colour() {
    let other = [0, 112, 204, 255];
    let result = report(&stroke(18, STROKE, true), &stroke(18, other, true));
    assert!(!result.passed);
    assert!(result.color_delta_e > COLOR_TOLERANCE, "{result:?}");
}

#[test]
fn the_mask_covers_the_icon_box_and_its_antialiasing_margin_only() {
    let mut mask = vec![false; 40 * 40];
    mark_icons(&mut mask, &layout(), 40, 40);
    let at = |x: usize, y: usize| mask[y * 40 + x];
    // The box is 12..28 physical; the margin adds two pixels on each side.
    assert!(at(10, 20) && at(29, 20));
    assert!(!at(9, 20) && !at(30, 20));
    assert!(!at(20, 9));
}
