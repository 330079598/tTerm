//! Grid geometry: sizes measured in cells, the pixel size of a cell, and
//! points on the grid.
//!
//! Absolute lines and screen rows are different things and stay different
//! types of number here. An absolute line is a coordinate into the grid and is
//! negative inside the scrollback; a screen row is an index into a snapshot
//! and always starts at zero (design note 4.2). The conversion between them
//! lives in [`crate::snapshot::Snapshot::screen_row`] so there is one
//! definition of it.

use std::ops::RangeInclusive;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point as AnsiPoint};

/// The number of columns and visible rows of a grid.
///
/// The dimensions are bounded because a transient zero from a window layout
/// must not panic a session: Alacritty indexes the last column and divides by
/// the line count. The constructor clamps instead of trusting the caller, and
/// because the fields are public, [`crate::terminal::Terminal`] clamps again
/// with [`GridSize::clamped`] before a size reaches Alacritty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridSize {
    /// Number of columns.
    pub columns: usize,
    /// Number of visible rows, excluding the scrollback above them.
    pub screen_lines: usize,
}

impl GridSize {
    /// The narrowest grid the model accepts.
    ///
    /// Design note 5.1 already keeps the view at two columns because
    /// Alacritty draws some emoji incorrectly in a one-column grid. The model
    /// enforces the same floor because Alacritty 0.26.0's reflow did not
    /// finish when the P0-09 crash test resized a terminal with wide
    /// characters in its history to one column (report P0-09, deviation 1).
    /// Clamping here makes that freeze unreachable for every caller.
    pub const MIN_COLUMNS: usize = 2;

    /// Creates a size, raising a narrow grid to [`GridSize::MIN_COLUMNS`] and
    /// a zero row count to one.
    pub const fn new(columns: usize, screen_lines: usize) -> Self {
        Self {
            columns: if columns < Self::MIN_COLUMNS {
                Self::MIN_COLUMNS
            } else {
                columns
            },
            screen_lines: if screen_lines == 0 { 1 } else { screen_lines },
        }
    }

    /// The same size with the floors of [`GridSize::new`] applied, for a value
    /// that may have been built from its public fields directly.
    pub const fn clamped(self) -> Self {
        Self::new(self.columns, self.screen_lines)
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// The size of one cell in device pixels, measured by the view.
///
/// The model only stores it: it is what a program gets back when it asks for
/// the text area size in pixels. A zero value means the view has not measured
/// the font yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CellSize {
    /// Width of one cell in device pixels.
    pub width: u16,
    /// Height of one cell in device pixels.
    pub height: u16,
}

impl CellSize {
    /// Creates a cell size from the view's font metrics.
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

/// The size of the text area in cells and pixels.
///
/// This is the shape a `CSI 14 t` reply needs; the caller fills it from a
/// snapshot when it answers a [`crate::event::Intent::WindowSizeRequest`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowSize {
    /// The grid the program is drawing into.
    pub grid: GridSize,
    /// The pixel size of one cell.
    pub cell: CellSize,
}

/// A point on the grid: an absolute line and a column.
///
/// Line zero is the first row of the screen; negative lines are above it in
/// the scrollback. A column is zero-based and always within the grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPoint {
    /// The absolute line.
    pub line: i32,
    /// The column, zero-based.
    pub column: usize,
}

impl GridPoint {
    /// Creates a point.
    pub const fn new(line: i32, column: usize) -> Self {
        Self { line, column }
    }
}

/// An inclusive range of cells on the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridRange {
    /// The first cell of the range.
    pub start: GridPoint,
    /// The last cell of the range.
    pub end: GridPoint,
}

impl GridRange {
    /// Creates a range from two corners, which are expected in reading order.
    pub const fn new(start: GridPoint, end: GridPoint) -> Self {
        Self { start, end }
    }

    /// Whether the point lies between the corners, both included.
    ///
    /// Within a multi-line range this covers every column of the middle
    /// lines, which is what a search match spanning a line break means.
    pub fn contains(&self, point: GridPoint) -> bool {
        self.start <= point && point <= self.end
    }
}

impl From<AnsiPoint> for GridPoint {
    fn from(point: AnsiPoint) -> Self {
        Self {
            line: point.line.0,
            column: point.column.0,
        }
    }
}

impl From<GridPoint> for AnsiPoint {
    fn from(point: GridPoint) -> Self {
        Self {
            line: Line(point.line),
            column: Column(point.column),
        }
    }
}

impl From<RangeInclusive<AnsiPoint>> for GridRange {
    fn from(range: RangeInclusive<AnsiPoint>) -> Self {
        Self {
            start: (*range.start()).into(),
            end: (*range.end()).into(),
        }
    }
}
