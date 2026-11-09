use super::*;
use crate::layout::{
    BoundsSnapshot, ElementSizing, ElementSnapshot, LayoutDocument, PaddingSnapshot, StyleSnapshot,
};
use image::{Rgba, RgbaImage};
use std::collections::BTreeMap;

const BACKGROUND: [u8; 4] = [27, 29, 35, 255];
const GREEN: [u8; 4] = [0, 120, 0, 255];
const RED: [u8; 4] = [200, 0, 0, 255];

fn image(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

fn fill(image: &mut RgbaImage, left: u32, top: u32, right: u32, bottom: u32, color: [u8; 4]) {
    for y in top..bottom {
        for x in left..right {
            image.put_pixel(x, y, Rgba(color));
        }
    }
}

fn style(
    background: [u8; 4],
    border: [u8; 4],
    border_width: f32,
    border_radius: f32,
) -> StyleSnapshot {
    StyleSnapshot {
        background_color: background,
        border_color: border,
        border_width,
        border_radius,
        ring_spread: 0.0,
        font_family: "system-ui".to_owned(),
        font_size: 14.0,
        font_weight: 400,
        line_height: 16.0,
        color: [204, 204, 204, 255],
        padding: PaddingSnapshot::all(0.0),
        declared_background_color: None,
        opacity: 1.0,
        declared_border_color: None,
    }
}

fn element(bounds: BoundsSnapshot, style: StyleSnapshot) -> ElementSnapshot {
    ElementSnapshot {
        id: "example.button".to_owned(),
        sizing: ElementSizing::Text,
        bounds,
        style,
    }
}

fn viewport() -> ViewportSnapshot {
    ViewportSnapshot {
        width: 40,
        height: 40,
        device_scale_factor: 1.0,
    }
}

#[test]
fn measures_background_and_border_from_pixels() {
    let mut image = image(40, 40, BACKGROUND);
    // The border is drawn first, then the interior covers all but the band.
    fill(&mut image, 8, 8, 32, 24, RED);
    fill(&mut image, 10, 10, 30, 22, GREEN);
    let element = element(
        BoundsSnapshot::new(8.0, 8.0, 24.0, 16.0),
        style([0, 0, 0, 0], [0, 0, 0, 0], 2.0, 0.0),
    );

    let measured = measure_element(&image, viewport(), &element, &[]).expect("measurement");

    assert_eq!(measured.background, GREEN);
    assert_eq!(measured.border, Some(RED));
}

#[test]
fn excludes_text_regions_from_the_background_mode() {
    let mut image = image(20, 20, [10, 10, 10, 255]);
    fill(&mut image, 0, 0, 16, 20, [255, 255, 255, 255]);
    let element = element(
        BoundsSnapshot::new(0.0, 0.0, 20.0, 20.0),
        style([10, 10, 10, 255], [0, 0, 0, 0], 0.0, 0.0),
    );
    let text = [BoundsSnapshot::new(0.0, 0.0, 16.0, 20.0)];

    let measured = measure_element(&image, viewport(), &element, &text).expect("measurement");

    assert_eq!(measured.background, [10, 10, 10, 255]);
    assert_eq!(measured.border, None);
}

#[test]
fn declared_colours_are_preserved_and_measured_colours_replace_them() {
    let image = image(20, 20, BACKGROUND);
    let element = element(
        BoundsSnapshot::new(0.0, 0.0, 20.0, 20.0),
        style([0, 0, 0, 255], [61, 61, 61, 255], 0.0, 0.0),
    );
    let mut document = LayoutDocument {
        version: crate::layout::SCHEMA_VERSION,
        screen: "example.button-input.default".to_owned(),
        viewport: ViewportSnapshot {
            width: 20,
            height: 20,
            device_scale_factor: 1.0,
        },
        elements: vec![element],
        text_regions: Vec::new(),
    };

    measure_document(&image, &mut document).expect("measurement");

    let style = &document.elements[0].style;
    assert_eq!(style.declared_background_color, Some([0, 0, 0, 255]));
    assert_eq!(style.declared_border_color, Some([61, 61, 61, 255]));
    assert_eq!(style.background_color, BACKGROUND);
    assert_eq!(style.border_color, [61, 61, 61, 255]);
}

#[test]
fn an_element_outside_the_image_names_the_element() {
    let image = image(20, 20, BACKGROUND);
    let element = element(
        BoundsSnapshot::new(100.0, 100.0, 10.0, 10.0),
        style([0, 0, 0, 255], [0, 0, 0, 0], 0.0, 0.0),
    );
    let mut document = LayoutDocument {
        version: crate::layout::SCHEMA_VERSION,
        screen: "example.button-input.default".to_owned(),
        viewport: ViewportSnapshot {
            width: 20,
            height: 20,
            device_scale_factor: 1.0,
        },
        elements: vec![element],
        text_regions: Vec::new(),
    };

    let error = measure_document(&image, &mut document).expect_err("out of bounds");

    assert!(error.contains("example.button"), "{error}");
    assert!(error.contains("no background pixels"), "{error}");
}

#[test]
fn mode_breaks_ties_by_the_smallest_colour() {
    let counts = BTreeMap::from([
        ([2_u8, 0, 0, 255], 3_u64),
        ([1_u8, 0, 0, 255], 3_u64),
        ([9_u8, 0, 0, 255], 1_u64),
    ]);

    assert_eq!(mode(counts), Some([1, 0, 0, 255]));
}

#[test]
fn a_borderless_element_has_no_measured_border() {
    let image = image(10, 10, BACKGROUND);
    let element = element(
        BoundsSnapshot::new(0.0, 0.0, 10.0, 10.0),
        style([0, 0, 0, 255], [61, 61, 61, 255], 0.0, 0.0),
    );

    let measured = measure_element(&image, viewport(), &element, &[]).expect("measurement");

    assert_eq!(measured.background, BACKGROUND);
    assert_eq!(measured.border, None);
}
