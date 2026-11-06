//! Image metrics: SSIM, CIEDE2000 colour difference, text masks and heatmaps.

use super::ColorReport;
use super::color::delta_e_2000;
use super::color::srgb_to_lab;
use crate::layout::LayoutDocument;
use image::{Rgba, RgbaImage};

/// The local SSIM window. Windows do not overlap, which keeps the comparison
/// deterministic and makes the heatmap readable as a block map.
const SSIM_WINDOW: u32 = 16;
/// A masked window needs at least this many unmasked pixels to be scored.
const SSIM_MIN_MASKED_PIXELS: u32 = 32;
/// Text bounds are grown by this many physical pixels before masking, so
/// antialiasing at the edge is left out of the comparison too.
const TEXT_MASK_DILATION: i32 = 2;

/// Mean SSIM over 16 by 16 windows, optionally ignoring masked pixels.
///
/// A window is skipped when the mask leaves fewer than 32 pixels in it. The
/// metric is the standard SSIM formula with C1 = (0.01·255)² and
/// C2 = (0.03·255)² on gamma-encoded luma; see `parity/README.md` for why the
/// tool does not reproduce a different SSIM implementation pixel for pixel.
pub fn ssim(reference: &RgbaImage, actual: &RgbaImage, mask: Option<&[bool]>) -> f64 {
    if reference.dimensions() != actual.dimensions() {
        return 0.0;
    }
    let (width, height) = reference.dimensions();
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    let mut total = 0.0;
    let mut windows = 0_u32;

    for window_y in (0..height).step_by(SSIM_WINDOW as usize) {
        for window_x in (0..width).step_by(SSIM_WINDOW as usize) {
            let mut sum_reference = 0.0;
            let mut sum_actual = 0.0;
            let mut sum_reference_sq = 0.0;
            let mut sum_actual_sq = 0.0;
            let mut sum_product = 0.0;
            let mut count = 0_u32;

            let end_y = (window_y + SSIM_WINDOW).min(height);
            let end_x = (window_x + SSIM_WINDOW).min(width);
            for y in window_y..end_y {
                for x in window_x..end_x {
                    let index = (y * width + x) as usize;
                    if let Some(mask) = mask
                        && mask.get(index).copied().unwrap_or(false)
                    {
                        continue;
                    }
                    let reference_luma = luma(reference.get_pixel(x, y).0);
                    let actual_luma = luma(actual.get_pixel(x, y).0);
                    sum_reference += reference_luma;
                    sum_actual += actual_luma;
                    sum_reference_sq += reference_luma * reference_luma;
                    sum_actual_sq += actual_luma * actual_luma;
                    sum_product += reference_luma * actual_luma;
                    count += 1;
                }
            }

            if count < SSIM_MIN_MASKED_PIXELS {
                continue;
            }
            let count = f64::from(count);
            let mean_reference = sum_reference / count;
            let mean_actual = sum_actual / count;
            let variance_reference =
                (sum_reference_sq / count - mean_reference * mean_reference).max(0.0);
            let variance_actual = (sum_actual_sq / count - mean_actual * mean_actual).max(0.0);
            let covariance = sum_product / count - mean_reference * mean_actual;
            let numerator = (2.0 * mean_reference * mean_actual + c1) * (2.0 * covariance + c2);
            let denominator = (mean_reference * mean_reference + mean_actual * mean_actual + c1)
                * (variance_reference + variance_actual + c2);
            total += numerator / denominator;
            windows += 1;
        }
    }

    if windows == 0 {
        0.0
    } else {
        (total / f64::from(windows)).min(1.0)
    }
}

/// Mean and maximum CIEDE2000 difference over the unmasked pixels.
///
/// Pixels with identical channels contribute zero without a colour-space
/// conversion; on a parity screenshot that is almost every pixel, so the
/// expensive conversion only runs where the two sides actually differ.
pub fn color_difference(reference: &RgbaImage, actual: &RgbaImage, mask: &[bool]) -> ColorReport {
    let (width, height) = reference.dimensions();
    let mut sum = 0.0;
    let mut max = 0.0_f64;
    let mut above_one = 0_u64;
    let mut count = 0_u64;

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            if mask.get(index).copied().unwrap_or(false) {
                continue;
            }
            let first = reference.get_pixel(x, y).0;
            let second = actual.get_pixel(x, y).0;
            count += 1;
            if first[0..3] == second[0..3] {
                continue;
            }
            let difference = delta_e_2000(srgb_to_lab(first), srgb_to_lab(second));
            sum += difference;
            max = max.max(difference);
            if difference > 1.0 {
                above_one += 1;
            }
        }
    }

    ColorReport {
        mean_delta_e: if count == 0 { 0.0 } else { sum / count as f64 },
        max_delta_e: max,
        pixels_above_one: above_one,
    }
}

/// Builds a heatmap: white where the images agree, red where they differ.
pub fn heatmap(reference: &RgbaImage, actual: &RgbaImage) -> RgbaImage {
    let (width, height) = reference.dimensions();
    let mut output = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let first = reference.get_pixel(x, y).0;
            let second = actual.get_pixel(x, y).0;
            let difference = (0..4)
                .map(|channel| first[channel].abs_diff(second[channel]))
                .max()
                .unwrap_or(0);
            let intensity = (u16::from(difference) * 8).min(255) as u8;
            output.put_pixel(x, y, Rgba([255, 255 - intensity, 255 - intensity, 255]));
        }
    }
    output
}

/// Marks every text rectangle from both layouts, scaled to device pixels.
pub fn text_mask(
    reference: &LayoutDocument,
    actual: &LayoutDocument,
    width: u32,
    height: u32,
) -> Vec<bool> {
    let scale = f64::from(reference.viewport.device_scale_factor);
    let mut mask = vec![false; (width * height) as usize];
    for region in reference
        .text_regions
        .iter()
        .chain(actual.text_regions.iter())
    {
        mark_region(&mut mask, width, height, region, scale);
    }
    mask
}

fn mark_region(
    mask: &mut [bool],
    width: u32,
    height: u32,
    region: &crate::layout::BoundsSnapshot,
    scale: f64,
) {
    let left = (f64::from(region.x) * scale).floor() as i32 - TEXT_MASK_DILATION;
    let top = (f64::from(region.y) * scale).floor() as i32 - TEXT_MASK_DILATION;
    let right = ((f64::from(region.x + region.width) * scale).ceil() as i32) + TEXT_MASK_DILATION;
    let bottom = ((f64::from(region.y + region.height) * scale).ceil() as i32) + TEXT_MASK_DILATION;
    let left = left.max(0);
    let top = top.max(0);
    let right = right.min(width as i32);
    let bottom = bottom.min(height as i32);
    if right <= left || bottom <= top {
        return;
    }
    for y in top..bottom {
        let row = (y as u32 * width) as usize;
        for x in left..right {
            mask[row + x as usize] = true;
        }
    }
}

fn luma(pixel: [u8; 4]) -> f64 {
    0.212_6 * f64::from(pixel[0]) + 0.715_2 * f64::from(pixel[1]) + 0.072_2 * f64::from(pixel[2])
}

#[cfg(test)]
mod tests;
