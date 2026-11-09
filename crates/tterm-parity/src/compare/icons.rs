//! The icon check (decided by stone on 2026-11-09).
//!
//! WebKit and GPUI place the same Lucide strokes but antialias their edges
//! differently, so icons cannot pass the per-pixel colour gate even when they
//! are identical in shape, size and position. Each reference icon is therefore
//! left out of that gate and checked here instead, inside the reference icon's
//! box on both screenshots:
//!
//! - ink: how far the pixels are from the background, summed; equal within
//!   [`INK_TOLERANCE`], so a missing, extra or thicker icon fails;
//! - centroid: the ink-weighted centre, within [`CENTROID_TOLERANCE`] logical
//!   pixels, so a moved icon fails;
//! - colour: the pixels the reference covers fully must have the same colour
//!   within CIEDE2000 [`COLOR_TOLERANCE`], so a recoloured icon fails.
//!
//! Only the reference records icons, so the GPUI side cannot vouch for itself.

use super::color::{delta_e_2000, srgb_to_lab};
use crate::layout::{BoundsSnapshot, IconSnapshot, LayoutDocument};
use image::RgbaImage;
use serde::Serialize;

/// Allowed relative difference of the ink totals.
pub const INK_TOLERANCE: f64 = 0.03;
/// Allowed centroid distance in logical pixels.
pub const CENTROID_TOLERANCE: f64 = 0.25;
/// Allowed CIEDE2000 between the fully covered stroke colours.
pub const COLOR_TOLERANCE: f64 = 1.0;
/// Physical pixels around the icon box that belong to it, for antialiasing
/// that spills over the box edge.
const MARGIN: i64 = 2;
/// A reference pixel counts as fully covered at this share of the most ink.
const FULL_COVERAGE: f64 = 0.9;

/// The result for one icon.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconReport {
    pub bounds: BoundsSnapshot,
    /// GPUI ink divided by reference ink.
    pub ink_ratio: f64,
    /// Centroid distance in logical pixels.
    pub centroid_delta: f64,
    /// CIEDE2000 between the mean fully covered colours.
    pub color_delta_e: f64,
    pub passed: bool,
}

/// The physical pixel rectangle an icon owns, clamped to the image.
fn region(icon: &IconSnapshot, scale: f64, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let b = icon.bounds;
    let left = ((f64::from(b.x) * scale).floor() as i64 - MARGIN).max(0) as u32;
    let top = ((f64::from(b.y) * scale).floor() as i64 - MARGIN).max(0) as u32;
    let right =
        (((f64::from(b.x + b.width) * scale).ceil() as i64 + MARGIN).max(0) as u32).min(width);
    let bottom =
        (((f64::from(b.y + b.height) * scale).ceil() as i64 + MARGIN).max(0) as u32).min(height);
    (left, top, right, bottom)
}

/// Leaves every reference icon out of the per-pixel colour gate.
pub fn mark_icons(mask: &mut [bool], reference: &LayoutDocument, width: u32, height: u32) {
    let scale = f64::from(reference.viewport.device_scale_factor);
    for icon in &reference.icons {
        let (left, top, right, bottom) = region(icon, scale, width, height);
        for y in top..bottom {
            for x in left..right {
                mask[(y * width + x) as usize] = true;
            }
        }
    }
}

/// The most common colour on the region's outer ring: the icon's background.
fn background(image: &RgbaImage, (left, top, right, bottom): (u32, u32, u32, u32)) -> [u8; 4] {
    let mut counts = std::collections::HashMap::new();
    for x in left..right {
        for y in [top, bottom.saturating_sub(1)] {
            *counts.entry(image.get_pixel(x, y).0).or_insert(0_u32) += 1;
        }
    }
    for y in top..bottom {
        for x in [left, right.saturating_sub(1)] {
            *counts.entry(image.get_pixel(x, y).0).or_insert(0_u32) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(color, count)| (*count, *color))
        .map_or([0, 0, 0, 255], |(color, _)| color)
}

/// Lab distance from the background: how much of the icon a pixel shows.
fn ink(pixel: [u8; 4], background: [f64; 3]) -> f64 {
    let lab = srgb_to_lab(pixel);
    ((lab[0] - background[0]).powi(2)
        + (lab[1] - background[1]).powi(2)
        + (lab[2] - background[2]).powi(2))
    .sqrt()
}

/// Checks every reference icon against the GPUI screenshot.
pub fn check_icons(
    reference: &RgbaImage,
    actual: &RgbaImage,
    layout: &LayoutDocument,
) -> Vec<IconReport> {
    let scale = f64::from(layout.viewport.device_scale_factor);
    let (width, height) = reference.dimensions();
    layout
        .icons
        .iter()
        .map(|icon| {
            check_icon(
                reference,
                actual,
                icon,
                scale,
                region(icon, scale, width, height),
            )
        })
        .collect()
}

fn check_icon(
    reference: &RgbaImage,
    actual: &RgbaImage,
    icon: &IconSnapshot,
    scale: f64,
    bounds: (u32, u32, u32, u32),
) -> IconReport {
    let (left, top, right, bottom) = bounds;
    let back = srgb_to_lab(background(reference, bounds));
    let mut sums = [[0.0_f64; 3]; 2];
    let mut samples = Vec::new();
    for y in top..bottom {
        for x in left..right {
            let first = reference.get_pixel(x, y).0;
            let second = actual.get_pixel(x, y).0;
            let weights = [ink(first, back), ink(second, back)];
            for (sum, weight) in sums.iter_mut().zip(weights) {
                sum[0] += weight;
                sum[1] += weight * f64::from(x);
                sum[2] += weight * f64::from(y);
            }
            samples.push((weights[0], first, second));
        }
    }
    let [reference_sum, actual_sum] = sums;
    let ink_ratio = if reference_sum[0] > 0.0 {
        actual_sum[0] / reference_sum[0]
    } else if actual_sum[0] > 0.0 {
        f64::INFINITY
    } else {
        1.0
    };
    let centroid = |sum: [f64; 3]| {
        if sum[0] > 0.0 {
            (sum[1] / sum[0], sum[2] / sum[0])
        } else {
            (0.0, 0.0)
        }
    };
    let (rx, ry) = centroid(reference_sum);
    let (ax, ay) = centroid(actual_sum);
    let centroid_delta = (rx - ax).hypot(ry - ay) / scale;
    let strongest = samples
        .iter()
        .map(|(weight, _, _)| *weight)
        .fold(0.0, f64::max);
    let mut means = [[0.0_f64; 3]; 2];
    let mut count = 0.0;
    for (weight, first, second) in &samples {
        if strongest > 0.0 && *weight >= strongest * FULL_COVERAGE {
            for (mean, pixel) in means.iter_mut().zip([first, second]) {
                for (channel, value) in mean.iter_mut().zip(pixel.iter()) {
                    *channel += f64::from(*value);
                }
            }
            count += 1.0;
        }
    }
    let to_pixel = |mean: [f64; 3]| {
        [
            (mean[0] / count).round() as u8,
            (mean[1] / count).round() as u8,
            (mean[2] / count).round() as u8,
            255,
        ]
    };
    let color_delta_e = if count > 0.0 {
        delta_e_2000(
            srgb_to_lab(to_pixel(means[0])),
            srgb_to_lab(to_pixel(means[1])),
        )
    } else {
        0.0
    };
    let passed = (ink_ratio - 1.0).abs() <= INK_TOLERANCE
        && centroid_delta <= CENTROID_TOLERANCE
        && color_delta_e <= COLOR_TOLERANCE;
    IconReport {
        bounds: icon.bounds,
        ink_ratio,
        centroid_delta,
        color_delta_e,
        passed,
    }
}

#[cfg(test)]
mod tests;
