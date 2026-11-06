use super::*;
use crate::layout::{BoundsSnapshot, LayoutDocument, ViewportSnapshot};
use image::Rgba;

fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

fn layout(text_regions: Vec<BoundsSnapshot>) -> LayoutDocument {
    LayoutDocument {
        version: 1,
        screen: "s".to_owned(),
        viewport: ViewportSnapshot {
            width: 10,
            height: 10,
            device_scale_factor: 2.0,
        },
        elements: Vec::new(),
        text_regions,
    }
}

#[test]
fn identical_images_score_one() {
    let image = solid(32, 32, [27, 29, 35, 255]);
    assert!((ssim(&image, &image, None) - 1.0).abs() < 1.0e-9);
}

#[test]
fn a_changed_window_lowers_ssim_and_turns_the_heatmap_red() {
    let reference = solid(32, 32, [255, 255, 255, 255]);
    let mut actual = reference.clone();
    for y in 0..16 {
        for x in 0..16 {
            actual.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let score = ssim(&reference, &actual, None);
    assert!(score < 0.8, "score was {score}");
    let map = heatmap(&reference, &actual);
    assert_eq!(map.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(map.get_pixel(31, 31).0, [255, 255, 255, 255]);
}

#[test]
fn masking_removes_the_differing_window() {
    let reference = solid(32, 32, [255, 255, 255, 255]);
    let mut actual = reference.clone();
    actual.put_pixel(5, 5, Rgba([255, 0, 0, 255]));
    let mut mask = vec![false; 32 * 32];
    for y in 0..16 {
        for x in 0..16 {
            mask[(y * 32 + x) as usize] = true;
        }
    }
    let masked = ssim(&reference, &actual, Some(&mask));
    let full = ssim(&reference, &actual, None);
    assert!((masked - 1.0).abs() < 1.0e-9, "masked was {masked}");
    assert!(full < 1.0, "full was {full}");
}

#[test]
fn images_of_different_sizes_score_zero() {
    let first = solid(16, 16, [255, 255, 255, 255]);
    let second = solid(32, 32, [255, 255, 255, 255]);
    assert_eq!(ssim(&first, &second, None), 0.0);
}

#[test]
fn a_text_region_is_scaled_dilated_and_clipped() {
    let reference = layout(vec![BoundsSnapshot::new(1.0, 1.0, 2.0, 2.0)]);
    let actual = layout(Vec::new());
    let mask = text_mask(&reference, &actual, 20, 20);
    assert!(mask[0], "the top-left pixel is inside the dilated region");
    assert!(mask[(7 * 20 + 7) as usize], "the region reaches 7 by 7");
    assert!(
        !mask[(9 * 20 + 9) as usize],
        "the region ends before 9 by 9"
    );
    assert_eq!(mask.iter().filter(|masked| **masked).count(), 64);
}

#[test]
fn identical_pixels_have_no_colour_difference() {
    let image = solid(16, 16, [0, 112, 204, 255]);
    let report = color_difference(&image, &image, &vec![false; 16 * 16]);
    assert_eq!(report.mean_delta_e, 0.0);
    assert_eq!(report.max_delta_e, 0.0);
    assert_eq!(report.pixels_above_one, 0);
}

#[test]
fn a_changed_pixel_is_counted() {
    let reference = solid(16, 16, [0, 112, 204, 255]);
    let mut actual = reference.clone();
    actual.put_pixel(3, 4, Rgba([204, 0, 0, 255]));
    let report = color_difference(&reference, &actual, &vec![false; 16 * 16]);
    assert!(report.mean_delta_e > 0.0);
    assert!(report.max_delta_e > 1.0);
    assert_eq!(report.pixels_above_one, 1);
}

#[test]
fn masked_pixels_are_left_out_of_the_colour_difference() {
    let reference = solid(16, 16, [0, 112, 204, 255]);
    let mut actual = reference.clone();
    actual.put_pixel(3, 4, Rgba([204, 0, 0, 255]));
    let mut mask = vec![false; 16 * 16];
    mask[4 * 16 + 3] = true;
    let report = color_difference(&reference, &actual, &mask);
    assert_eq!(report.max_delta_e, 0.0);
    assert_eq!(report.pixels_above_one, 0);
}
