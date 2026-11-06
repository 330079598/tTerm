// Derived from Alacritty's `alacritty/src/renderer/text/builtin_font.rs`,
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the pixel canvas becomes a list of rectangles and clipped
// polygons. Line bounds, stroke sizes and the arc and line shapes follow
// Alacritty's Canvas methods.

//! Geometry helpers shared by the built-in glyph families.
//!
//! All coordinates are cell-local pixels. A [`Geometry`] clips every shape to
//! the cell, so the drawing layer never has to trim and the tests can assert
//! that no glyph paints outside its cell.

use super::GlyphShape;
use super::Point;
use super::Rect;

/// A cell-sized shape list under construction.
pub(crate) struct Geometry {
    width: f32,
    height: f32,
    shapes: Vec<GlyphShape>,
}

impl Geometry {
    /// Starts a shape list for a cell.
    pub(crate) fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            shapes: Vec::new(),
        }
    }

    /// The horizontal center of the cell.
    pub(crate) fn center_x(&self) -> f32 {
        self.width / 2.0
    }

    /// The vertical center of the cell.
    pub(crate) fn center_y(&self) -> f32 {
        self.height / 2.0
    }

    /// Adds a rectangle, clipped to the cell.
    pub(crate) fn rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let left = x.max(0.0);
        let top = y.max(0.0);
        let right = (x + width).min(self.width);
        let bottom = (y + height).min(self.height);
        if right - left > f32::EPSILON && bottom - top > f32::EPSILON {
            self.shapes.push(GlyphShape::Rect(Rect {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            }));
        }
    }

    /// Adds a filled polygon, clipped to the cell.
    pub(crate) fn polygon(&mut self, points: Vec<Point>) {
        let mut points = points;
        points = clip_vertical(points, 0.0, true);
        points = clip_vertical(points, self.width, false);
        points = clip_horizontal(points, 0.0, true);
        points = clip_horizontal(points, self.height, false);
        if points.len() >= 3 {
            self.shapes.push(GlyphShape::Polygon(points));
        }
    }

    /// Draws a horizontal line centered on `y` with the given thickness.
    pub(crate) fn h_line(&mut self, x: f32, y: f32, length: f32, thickness: f32) {
        let (start, end) = h_line_bounds(y, thickness, self.height);
        self.rect(x, start, length, end - start);
    }

    /// Draws a vertical line centered on `x` with the given thickness.
    pub(crate) fn v_line(&mut self, x: f32, y: f32, length: f32, thickness: f32) {
        let (start, end) = v_line_bounds(x, thickness, self.width);
        self.rect(start, y, end - start, length);
    }

    /// Mirrors every shape horizontally within the cell.
    ///
    /// The Powerline right-hand glyphs are the left-hand ones mirrored, as in
    /// Alacritty's `Canvas::flip_horizontal`.
    pub(crate) fn flip_horizontal(&mut self) {
        for shape in &mut self.shapes {
            match shape {
                GlyphShape::Rect(rect) => {
                    rect.x = self.width - rect.x - rect.width;
                }
                GlyphShape::Polygon(points) => {
                    for point in points {
                        point.x = self.width - point.x;
                    }
                }
            }
        }
    }

    /// Finishes the list.
    pub(crate) fn finish(self) -> Vec<GlyphShape> {
        self.shapes
    }
}

/// The stroke width Alacritty derives from the cell width: one eighth of it,
/// rounded, at least one pixel.
pub(crate) fn stroke_size(width: f32) -> f32 {
    (width / 8.0).round().max(1.0)
}

/// The vertical extent of a horizontal line centered on `y`.
pub(crate) fn h_line_bounds(y: f32, thickness: f32, height: f32) -> (f32, f32) {
    let start = (y - thickness / 2.0).floor().max(0.0);
    let end = (y + thickness / 2.0).floor().min(height);
    (start, end)
}

/// The horizontal extent of a vertical line centered on `x`.
pub(crate) fn v_line_bounds(x: f32, thickness: f32, width: f32) -> (f32, f32) {
    let start = (x - thickness / 2.0).floor().max(0.0);
    let end = (x + thickness / 2.0).floor().min(width);
    (start, end)
}

/// A stroked line as a four-point polygon.
pub(crate) fn line_polygon(from: Point, to: Point, thickness: f32) -> Vec<Point> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return Vec::new();
    }
    let half = thickness / 2.0;
    let nx = -dy / length * half;
    let ny = dx / length * half;
    vec![
        Point::new(from.x + nx, from.y + ny),
        Point::new(to.x + nx, to.y + ny),
        Point::new(to.x - nx, to.y - ny),
        Point::new(from.x - nx, from.y - ny),
    ]
}

/// A stroked circular arc as a closed polygon.
///
/// Angles are in radians and grow clockwise because the y axis points down.
pub(crate) fn arc_polygon(
    center: Point,
    radius: f32,
    start: f32,
    end: f32,
    thickness: f32,
) -> Vec<Point> {
    const SEGMENTS: usize = 12;
    let outer = radius + thickness / 2.0;
    let inner = (radius - thickness / 2.0).max(0.0);
    let mut points = Vec::with_capacity(2 * (SEGMENTS + 1));
    for step in 0..=SEGMENTS {
        let angle = start + (end - start) * step as f32 / SEGMENTS as f32;
        points.push(Point::new(
            center.x + outer * angle.cos(),
            center.y + outer * angle.sin(),
        ));
    }
    for step in (0..=SEGMENTS).rev() {
        let angle = start + (end - start) * step as f32 / SEGMENTS as f32;
        points.push(Point::new(
            center.x + inner * angle.cos(),
            center.y + inner * angle.sin(),
        ));
    }
    points
}

/// Clips a polygon against a vertical line.
fn clip_vertical(points: Vec<Point>, boundary: f32, keep_right: bool) -> Vec<Point> {
    let inside = |point: &Point| {
        if keep_right {
            point.x >= boundary
        } else {
            point.x <= boundary
        }
    };
    clip(points, inside, |from, to| {
        let dx = to.x - from.x;
        if dx.abs() <= f32::EPSILON {
            return from;
        }
        let t = (boundary - from.x) / dx;
        Point::new(boundary, from.y + (to.y - from.y) * t)
    })
}

/// Clips a polygon against a horizontal line.
fn clip_horizontal(points: Vec<Point>, boundary: f32, keep_below: bool) -> Vec<Point> {
    let inside = |point: &Point| {
        if keep_below {
            point.y >= boundary
        } else {
            point.y <= boundary
        }
    };
    clip(points, inside, |from, to| {
        let dy = to.y - from.y;
        if dy.abs() <= f32::EPSILON {
            return from;
        }
        let t = (boundary - from.y) / dy;
        Point::new(from.x + (to.x - from.x) * t, boundary)
    })
}

/// Sutherland-Hodgman clipping against one half-plane.
fn clip(
    points: Vec<Point>,
    inside: impl Fn(&Point) -> bool,
    intersect: impl Fn(Point, Point) -> Point,
) -> Vec<Point> {
    let Some(&last) = points.last() else {
        return Vec::new();
    };
    let mut result = Vec::with_capacity(points.len() + 1);
    let mut previous = last;
    let mut previous_inside = inside(&previous);
    for current in points {
        let current_inside = inside(&current);
        if current_inside {
            if !previous_inside {
                result.push(intersect(previous, current));
            }
            result.push(current);
        } else if previous_inside {
            result.push(intersect(previous, current));
        }
        previous = current;
        previous_inside = current_inside;
    }
    result
}
