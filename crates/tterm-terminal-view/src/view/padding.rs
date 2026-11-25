//! Padding changes refit the content rectangle without touching the session.

use gpui_kit::{Bounds, Context, Pixels};

use super::TerminalView;
use super::font::grid_for;
use super::layout::{self, TerminalPadding};

impl TerminalView {
    /// The inset content rectangle used by drawing and pointer input.
    ///
    /// It is `None` before the first prepared frame.
    pub fn content_bounds(&self) -> Option<Bounds<Pixels>> {
        self.last_bounds
            .map(|bounds| layout::content_bounds(bounds, self.padding))
    }

    /// The padding this view draws and hit-tests with.
    pub fn padding(&self) -> TerminalPadding {
        self.padding
    }

    /// Replaces the content padding and refits a visible pane.
    ///
    /// A hidden pane keeps the new padding and measures when it is shown, as
    /// the font path does. A visible pane either recomputes its grid from the
    /// last measured cell and tells the session, or repaints when the grid did
    /// not change. The padding is never added anywhere else, so a pane cannot
    /// count it twice.
    pub fn set_padding(&mut self, padding: TerminalPadding, cx: &mut Context<Self>) {
        if self.padding == padding {
            return;
        }
        self.padding = padding;
        if self.visible {
            self.refit_grid();
        }
        self.invalidate(cx);
    }

    /// Repaints after a committed settings save changed a model option the
    /// view does not draw through its own configuration, such as the default
    /// cursor shape.
    pub fn notify_settings_changed(&mut self, cx: &mut Context<Self>) {
        self.invalidate(cx);
    }

    /// Recomputes the grid from a padding change without re-measuring the cell.
    ///
    /// This only runs for a visible pane. It keeps the session and the view in
    /// agreement before the next frame, including when the new grid is the
    /// same and only the content origin moved.
    fn refit_grid(&mut self) {
        let (Some(bounds), Some(geometry)) = (self.last_bounds, self.geometry) else {
            return;
        };
        let content = layout::content_bounds(bounds, self.padding);
        let grid = grid_for(
            content.size.width,
            content.size.height,
            &geometry,
            self.terminal.scrollback() > 0,
        );
        if self.last_grid == Some(grid) {
            return;
        }
        self.terminal.resize(grid, geometry.model_cell_size());
        if let Some(resize) = &self.resize {
            resize(grid, geometry.model_cell_size());
        }
        self.last_grid = Some(grid);
        self.stats.resizes = self.stats.resizes.saturating_add(1);
        self.last_frame = None;
    }
}
