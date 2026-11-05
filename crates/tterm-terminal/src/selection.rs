//! Selections: what the user selected, in absolute grid coordinates.
//!
//! The state itself (anchor, expansion rules, rotation when the screen
//! scrolls or resizes) is Alacritty's. This module only mirrors the parts the
//! view needs to read, so the snapshot can hand out a plain range instead of
//! Alacritty's selection object.

use alacritty_terminal::index::Direction as AnsiSide;
use alacritty_terminal::selection::{
    SelectionRange as AnsiSelectionRange, SelectionType as AnsiSelectionType,
};

use crate::grid::GridPoint;

/// How a selection expands from the point the user clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    /// Exactly the cells between the corners.
    Simple,
    /// A rectangle between the corners.
    Block,
    /// Whole words: the anchor expands to the nearest word boundary.
    Semantic,
    /// Whole lines.
    Lines,
}

/// Which side of a cell a selection endpoint sits on.
///
/// The side decides whether the cell under the pointer is part of the range
/// when a drag ends there, which is what makes a selection feel precise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The endpoint is at the left edge of the cell, so the cell is included.
    Left,
    /// The endpoint is at the right edge of the cell, so the cell is excluded.
    Right,
}

impl From<Side> for AnsiSide {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
        }
    }
}

impl From<SelectionKind> for AnsiSelectionType {
    fn from(kind: SelectionKind) -> Self {
        match kind {
            SelectionKind::Simple => Self::Simple,
            SelectionKind::Block => Self::Block,
            SelectionKind::Semantic => Self::Semantic,
            SelectionKind::Lines => Self::Lines,
        }
    }
}

/// The cells a selection covers, in absolute grid coordinates.
///
/// `start` is above and to the left of `end`, so both corners are in reading
/// order. Coordinates may be negative: a selection survives scrolling, and
/// `resize` or scrollback eviction can move parts of it outside the viewport.
/// Only the cells whose screen row falls inside the frame are drawn from a
/// snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectionRange {
    /// The first selected cell.
    pub start: GridPoint,
    /// The last selected cell.
    pub end: GridPoint,
    /// Whether the selection is a rectangle rather than a reading-order span.
    pub block: bool,
}

impl SelectionRange {
    /// Whether the cell at `point` is selected.
    ///
    /// A reading-order selection covers the middle lines completely and the
    /// first and last line only between the endpoints; a block selection
    /// keeps the same columns on every line.
    pub fn contains(&self, point: GridPoint) -> bool {
        if point < self.start || point > self.end {
            return false;
        }
        if self.block {
            return self.start.column <= point.column && point.column <= self.end.column;
        }
        let after_start = point.line != self.start.line || point.column >= self.start.column;
        let before_end = point.line != self.end.line || point.column <= self.end.column;
        after_start && before_end
    }
}

impl From<AnsiSelectionRange> for SelectionRange {
    fn from(range: AnsiSelectionRange) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
            block: range.is_block,
        }
    }
}
