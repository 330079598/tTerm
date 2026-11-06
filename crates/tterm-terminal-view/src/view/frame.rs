//! The immutable frame the element paints, and the paint entry point.

use std::sync::Arc;

use gpui_kit::{Bounds, ContentMask, Pixels, Point, Window};
use tterm_terminal::grid::GridSize;
use tterm_terminal::palette::{Rgba, TerminalPalette};
use tterm_terminal::snapshot::Snapshot;

use super::font::CellGeometry;
use super::paint::{CursorPaint, PaintContext};
use super::row::RowLayout;
use super::style::hsla;

/// One snapshot plus its shaped rows.
pub struct FrameData {
    /// The snapshot the rows were built from.
    pub snapshot: Snapshot,
    /// One shaped layout per visible row.
    pub rows: Vec<Arc<RowLayout>>,
    /// The cell measurements used for this frame.
    pub metrics: CellGeometry,
    /// The palette used for this frame.
    pub palette: TerminalPalette,
}

impl FrameData {
    /// The grid size of the snapshot.
    pub fn grid(&self) -> GridSize {
        self.snapshot.grid()
    }
}

/// What an element's prepaint found to paint.
pub enum PreparedFrame {
    /// The view is hidden; only the terminal background is painted.
    Empty {
        /// The palette's background.
        background: Rgba,
    },
    /// A full frame.
    Full {
        /// The snapshot and shaped rows.
        data: Arc<FrameData>,
        /// The top-left corner of the grid.
        origin: Point<Pixels>,
        /// The cursor, after focus and blink, when one is drawn.
        cursor: Option<CursorPaint>,
        /// Whether the cursor is in its visible blink phase.
        show_cursor: bool,
    },
}

impl PreparedFrame {
    /// Paints the frame into `bounds`.
    pub fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        match self {
            Self::Empty { background } => {
                window.paint_quad(gpui_kit::fill(bounds, hsla(*background)));
            }
            Self::Full {
                data,
                origin,
                cursor,
                show_cursor,
            } => {
                let mask = ContentMask { bounds };
                window.with_content_mask(Some(mask), |window| {
                    let mut context = PaintContext {
                        window,
                        origin: *origin,
                        metrics: data.metrics,
                        palette: &data.palette,
                        selection: data.snapshot.selection(),
                        display_offset: data.snapshot.display_offset(),
                        show_cursor: *show_cursor,
                        cursor: *cursor,
                    };
                    context.paint_terminal_background(bounds);
                    for (index, row) in data.rows.iter().enumerate() {
                        context.paint_row(index, row);
                    }
                    if let Some(cursor) = context.cursor
                        && let Some(row) = data.rows.get(cursor.row)
                    {
                        context.paint_cursor(row);
                    }
                });
            }
        }
    }
}

/// Builds the cursor from a snapshot, when it falls inside the viewport.
pub fn cursor_for(snapshot: &Snapshot, focused: bool) -> Option<CursorPaint> {
    let cursor = snapshot.cursor();
    if !cursor.is_visible() {
        return None;
    }
    let row = snapshot.screen_row(cursor.point.line)?;
    let grid = snapshot.grid();
    if cursor.point.column >= grid.columns {
        return None;
    }
    let width_cells = snapshot
        .row(row)
        .and_then(|row| row.cell(cursor.point.column))
        .map(|cell| if cell.attributes.wide { 2 } else { 1 })
        .unwrap_or(1)
        .min(grid.columns.saturating_sub(cursor.point.column).max(1));
    Some(CursorPaint {
        row,
        column: cursor.point.column,
        width_cells,
        shape: cursor.shape,
        focused,
    })
}
