//! Measures the colours a screen actually painted.
//!
//! Element colours in the layout JSON are not copied from the view's
//! `StyleSnapshot`. This module samples the screenshot instead:
//!
//! - background: the most common pixel inside the rounded border box, after
//!   removing the border band, a one-pixel antialiasing margin and every text
//!   rectangle (dilated by two physical pixels);
//! - border: the most common pixel in the middle of each straight border side,
//!   excluding the rounded corners.
//!
//! A view can therefore still declare a colour for documentation, but the
//! comparison uses what the renderer produced. Transparent declarations keep
//! their alpha because the pixels only contain the composite; the declared
//! value is preserved in `declaredBackgroundColor`/`declaredBorderColor` for the
//! cross-check.

use crate::layout::{BoundsSnapshot, ElementSnapshot, LayoutDocument, ViewportSnapshot};
use image::RgbaImage;
use std::collections::BTreeMap;

/// The colours measured for one element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasuredColors {
    /// Composited background colour.
    pub background: [u8; 4],
    /// Border colour when the element declares a border.
    pub border: Option<[u8; 4]>,
}

/// Fills in measured colours for every element of `document`.
pub fn measure_document(image: &RgbaImage, document: &mut LayoutDocument) -> Result<(), String> {
    let viewport = document.viewport;
    let text_regions = document.text_regions.clone();
    for element in &mut document.elements {
        let measured = measure_element(image, viewport, element, &text_regions)
            .map_err(|error| format!("{}: {error}", element.id))?;
        let declared_background = element
            .style
            .declared_background_color
            .unwrap_or(element.style.background_color);
        let declared_border = element
            .style
            .declared_border_color
            .unwrap_or(element.style.border_color);
        element.style.declared_background_color = Some(declared_background);
        element.style.declared_border_color = Some(declared_border);
        element.style.background_color = measured.background;
        if let Some(border) = measured.border {
            element.style.border_color = border;
        }
    }
    Ok(())
}

/// Measures one element. The caller adds the element id to any error.
pub fn measure_element(
    image: &RgbaImage,
    viewport: ViewportSnapshot,
    element: &ElementSnapshot,
    text_regions: &[BoundsSnapshot],
) -> Result<MeasuredColors, String> {
    let scale = viewport.device_scale_factor;
    if !(scale.is_finite() && scale > 0.0) {
        return Err(format!("invalid device scale factor {scale}"));
    }
    let bounds = element.bounds;
    let left = (bounds.x * scale).round() as i32;
    let top = (bounds.y * scale).round() as i32;
    let right = ((bounds.x + bounds.width) * scale).round() as i32;
    let bottom = ((bounds.y + bounds.height) * scale).round() as i32;
    if right <= left || bottom <= top {
        return Err(format!(
            "bounds {}x{} at {},{} are empty",
            bounds.width, bounds.height, bounds.x, bounds.y
        ));
    }

    let width = (right - left) as f32;
    let height = (bottom - top) as f32;
    let radius = (element.style.border_radius * scale)
        .max(0.0)
        .min(width / 2.0)
        .min(height / 2.0);
    let band = (element.style.border_width * scale).ceil().max(0.0) as i32;
    let region = BoxRegion {
        image,
        left,
        top,
        right,
        bottom,
        width,
        height,
        radius,
        band,
    };
    let background = region
        .background(text_regions, scale)
        .map_err(|error| format!("no background pixels: {error}"))?;
    let border = if element.style.border_width > 0.0 {
        region.border()?
    } else {
        None
    };

    Ok(MeasuredColors { background, border })
}

/// The physical-pixel rectangle of one element.
struct BoxRegion<'a> {
    image: &'a RgbaImage,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    width: f32,
    height: f32,
    radius: f32,
    band: i32,
}

impl BoxRegion<'_> {
    fn background(&self, text_regions: &[BoundsSnapshot], scale: f32) -> Result<[u8; 4], String> {
        let inset = self.band as f32 + 1.0;
        let mut counts = BTreeMap::new();
        for y in self.top.max(0)..self.bottom.min(self.image.height() as i32) {
            for x in self.left.max(0)..self.right.min(self.image.width() as i32) {
                let center_x = x as f32 + 0.5 - self.left as f32;
                let center_y = y as f32 + 0.5 - self.top as f32;
                if !inside_rounded_rect(
                    center_x,
                    center_y,
                    self.width,
                    self.height,
                    self.radius,
                    inset,
                ) {
                    continue;
                }
                if inside_any_text(x, y, text_regions, scale) {
                    continue;
                }
                let color = self.image.get_pixel(x as u32, y as u32).0;
                *counts.entry(color).or_insert(0_u64) += 1;
            }
        }

        if counts.is_empty() {
            // Small elements can be thinner than the border and the antialiasing
            // margin. Fall back to every non-text pixel of the box.
            for y in self.top.max(0)..self.bottom.min(self.image.height() as i32) {
                for x in self.left.max(0)..self.right.min(self.image.width() as i32) {
                    if inside_any_text(x, y, text_regions, scale) {
                        continue;
                    }
                    let color = self.image.get_pixel(x as u32, y as u32).0;
                    *counts.entry(color).or_insert(0_u64) += 1;
                }
            }
        }

        mode(counts).ok_or_else(|| "the element bounds contain no pixels".to_owned())
    }

    fn border(&self) -> Result<Option<[u8; 4]>, String> {
        if self.band <= 0 {
            return Ok(None);
        }
        let corner = self.radius.round() as i32 + 2;
        let horizontal_start = (self.left + corner).min(self.right);
        let horizontal_end = (self.right - corner).max(horizontal_start);
        let vertical_start = (self.top + corner).min(self.bottom);
        let vertical_end = (self.bottom - corner).max(vertical_start);
        let mut counts = BTreeMap::new();

        if horizontal_end > horizontal_start {
            self.sample_band(
                horizontal_start,
                self.top,
                horizontal_end,
                (self.top + self.band).min(self.bottom),
                &mut counts,
            );
            self.sample_band(
                horizontal_start,
                (self.bottom - self.band).max(self.top),
                horizontal_end,
                self.bottom,
                &mut counts,
            );
        }
        if vertical_end > vertical_start {
            self.sample_band(
                self.left,
                vertical_start,
                (self.left + self.band).min(self.right),
                vertical_end,
                &mut counts,
            );
            self.sample_band(
                (self.right - self.band).max(self.left),
                vertical_start,
                self.right,
                vertical_end,
                &mut counts,
            );
        }
        if counts.is_empty() {
            // The box is smaller than the corner exclusion; sample the whole
            // border box rather than reporting no border colour.
            self.sample_band(self.left, self.top, self.right, self.bottom, &mut counts);
        }

        let color = mode(counts).ok_or_else(|| {
            format!(
                "the {}x{} border box at {},{} contains no pixels",
                self.width, self.height, self.left, self.top
            )
        })?;
        Ok(Some(color))
    }

    fn sample_band(
        &self,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        counts: &mut BTreeMap<[u8; 4], u64>,
    ) {
        let image_right = self.image.width() as i32;
        let image_bottom = self.image.height() as i32;
        for y in top.max(0)..bottom.min(image_bottom) {
            for x in left.max(0)..right.min(image_right) {
                let color = self.image.get_pixel(x as u32, y as u32).0;
                *counts.entry(color).or_insert(0_u64) += 1;
            }
        }
    }
}

fn inside_rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32, inset: f32) -> bool {
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let radius = radius.min(half_width).min(half_height);
    let qx = (x - half_width).abs() - (half_width - radius);
    let qy = (y - half_height).abs() - (half_height - radius);
    let outside = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0);
    outside <= -inset
}

fn inside_any_text(x: i32, y: i32, text_regions: &[BoundsSnapshot], scale: f32) -> bool {
    text_regions.iter().any(|region| {
        let left = (region.x * scale).floor() as i32 - 2;
        let top = (region.y * scale).floor() as i32 - 2;
        let right = ((region.x + region.width) * scale).ceil() as i32 + 2;
        let bottom = ((region.y + region.height) * scale).ceil() as i32 + 2;
        x >= left && x < right && y >= top && y < bottom
    })
}

fn mode(counts: BTreeMap<[u8; 4], u64>) -> Option<[u8; 4]> {
    counts
        .into_iter()
        .max_by(|first, second| first.1.cmp(&second.1).then_with(|| second.0.cmp(&first.0)))
        .map(|(color, _)| color)
}

#[cfg(test)]
mod tests;
