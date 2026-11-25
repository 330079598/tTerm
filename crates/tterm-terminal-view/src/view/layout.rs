//! The inset content rectangle the grid is drawn and hit-tested in.
//!
//! The reference client gives the terminal page left, right and bottom padding
//! and no top padding. Drawing, the scrollbar and pointer input all read the
//! same [`content_bounds`] so a pointer in the padding can never become a
//! phantom cell or an outgoing mouse report.

use gpui_kit::{Bounds, Pixels, Point, point, px, size};
use tterm_terminal::grid::GridSize;

use super::font::CellGeometry;

/// Padding around the cell grid, in logical pixels.
///
/// It mirrors the three page-padding settings. The top edge is deliberately
/// absent: the reference client has no top padding, so the grid starts at the
/// element's top edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalPadding {
    /// Space between the element's left edge and the grid.
    pub left: u16,
    /// Space between the grid and the element's right edge.
    pub right: u16,
    /// Space below the grid.
    pub bottom: u16,
}

impl TerminalPadding {
    /// Creates padding from the three page-padding settings.
    pub const fn new(left: u16, right: u16, bottom: u16) -> Self {
        Self {
            left,
            right,
            bottom,
        }
    }
}

/// The element rectangle left for the grid after padding.
///
/// A view can be narrower or shorter than its configured padding. The insets
/// are clamped so the rectangle never leaves the element: the left inset wins
/// first, then the right and the bottom, and the content may collapse to a
/// zero-sized rectangle. The minimum grid still keeps a tiny view alive, and
/// the hit test intersects with this rectangle.
pub fn content_bounds(bounds: Bounds<Pixels>, padding: TerminalPadding) -> Bounds<Pixels> {
    let width = bounds.size.width.as_f32().max(0.0);
    let height = bounds.size.height.as_f32().max(0.0);
    let left = f32::from(padding.left).min(width);
    let right = f32::from(padding.right).min(width - left);
    let bottom = f32::from(padding.bottom).min(height);
    Bounds {
        origin: point(bounds.origin.x + px(left), bounds.origin.y),
        size: size(px(width - left - right), px(height - bottom)),
    }
}

/// The grid's top-left corner inside the content rectangle.
///
/// The x coordinate is snapped to a device pixel, as before. A bottom-anchored
/// viewport keeps its last row on the content rectangle's bottom edge, which
/// preserves the bottom padding.
pub fn grid_origin(
    content: Bounds<Pixels>,
    grid: GridSize,
    metrics: &CellGeometry,
    bottom_anchored: bool,
    scale_factor: f32,
) -> Point<Pixels> {
    let snap = |value: f32| (value * scale_factor).round() / scale_factor;
    let x = snap(content.origin.x.as_f32());
    let extra = if bottom_anchored {
        (content.size.height.as_f32() - grid.screen_lines as f32 * metrics.cell_height).max(0.0)
    } else {
        0.0
    };
    Point::new(px(x), px(snap(content.origin.y.as_f32() + extra)))
}

/// The cell under a pointer position, or `None` outside the grid.
///
/// The check is deliberately exclusive at the bottom and right edges: the
/// padding, the scrollbar gutter and the unused remainder of a partial cell
/// are not cells. Returning `None` there is what keeps pointer input from
/// inventing a cell or reporting a mouse event to the program.
pub(super) fn cell_at(
    content: Bounds<Pixels>,
    origin: Point<Pixels>,
    grid: GridSize,
    metrics: &CellGeometry,
    position: Point<Pixels>,
) -> Option<(usize, usize)> {
    if content.size.width <= px(0.0) || content.size.height <= px(0.0) {
        return None;
    }
    let grid_right = origin.x + px(grid.columns as f32 * metrics.cell_width);
    let grid_bottom = origin.y + px(grid.screen_lines as f32 * metrics.cell_height);
    let inside_content = position.x >= content.origin.x
        && position.x < content.right()
        && position.y >= content.origin.y
        && position.y < content.bottom();
    let inside_grid = position.x >= origin.x
        && position.x < grid_right
        && position.y >= origin.y
        && position.y < grid_bottom;
    if !inside_content || !inside_grid {
        return None;
    }
    let column = ((position.x - origin.x).as_f32() / metrics.cell_width).floor();
    let row = ((position.y - origin.y).as_f32() / metrics.cell_height).floor();
    let column = column.clamp(0.0, (grid.columns - 1) as f32) as usize;
    let row = row.clamp(0.0, (grid.screen_lines - 1) as f32) as usize;
    Some((row, column))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, px, size};

    fn element() -> Bounds<Pixels> {
        Bounds {
            origin: point(px(20.0), px(30.0)),
            size: size(px(800.0), px(600.0)),
        }
    }

    fn metrics() -> CellGeometry {
        CellGeometry {
            cell_width: 8.0,
            cell_height: 16.0,
            font_size: 14.0,
            baseline: 12.0,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn asymmetric_padding_leaves_the_expected_content_rectangle() {
        let content = content_bounds(element(), TerminalPadding::new(11, 7, 5));
        assert_eq!(content.origin, point(px(31.0), px(30.0)));
        assert_eq!(content.size, size(px(782.0), px(595.0)));
        let grid = super::super::font::grid_for(
            content.size.width,
            content.size.height,
            &metrics(),
            false,
        );
        assert_eq!((grid.columns, grid.screen_lines), (97, 37));
        assert_eq!(
            grid_origin(content, grid, &metrics(), false, 1.0),
            point(px(31.0), px(30.0))
        );
    }

    #[test]
    fn bottom_anchoring_keeps_the_bottom_padding() {
        let content = content_bounds(element(), TerminalPadding::new(11, 7, 5));
        let grid = GridSize::new(97, 37);
        let origin = grid_origin(content, grid, &metrics(), true, 1.0);
        assert_eq!(origin.y, px(33.0));
        assert_eq!(
            origin.y + px(grid.screen_lines as f32 * metrics().cell_height),
            content.bottom()
        );
    }

    #[test]
    fn tiny_views_clamp_the_insets_instead_of_leaving_the_element() {
        let tiny = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(10.0), px(6.0)),
        };
        let content = content_bounds(tiny, TerminalPadding::new(20, 20, 20));
        assert_eq!(content.origin, point(px(10.0), px(0.0)));
        assert_eq!(content.size, size(px(0.0), px(0.0)));
        assert_eq!(
            cell_at(
                content,
                content.origin,
                GridSize::new(2, 1),
                &metrics(),
                point(px(9.0), px(1.0))
            ),
            None
        );
    }

    #[test]
    fn padding_and_partial_cells_are_not_hit_as_cells() {
        let content = content_bounds(element(), TerminalPadding::new(11, 7, 5));
        // 782 is not a multiple of 8, so the last six content pixels are slack
        // between the grid's right edge and the content rectangle.
        let grid = super::super::font::grid_for(
            content.size.width,
            content.size.height,
            &metrics(),
            false,
        );
        let origin = grid_origin(content, grid, &metrics(), false, 1.0);
        let inside =
            |x: f32, y: f32| cell_at(content, origin, grid, &metrics(), point(px(x), px(y)));
        assert_eq!(inside(20.0, 30.0), None, "left padding");
        assert_eq!(inside(815.0, 30.0), None, "right padding");
        assert_eq!(inside(31.0, 625.0), None, "bottom padding");
        assert_eq!(inside(780.0, 100.0), Some((4, 93)), "inside the grid");
        assert_eq!(
            inside(810.0, 100.0),
            None,
            "the unused remainder is not a phantom cell"
        );
    }
}
