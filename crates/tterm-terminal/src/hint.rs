//! URL and path recognition over a visible frame.
//!
//! The rules are Alacritty's `display/hint.rs`: find the configured URL
//! regular expression in the viewport, join wrapped rows first, and shorten a
//! match at unbalanced closing brackets and at punctuation that is unlikely to
//! be part of the link. Alacritty has no bare-path matcher, so the path rule
//! is tTerm's own and deliberately conservative; the report says so.
//!
//! The module only reads a [`Snapshot`], so it needs no lock and no GPUI. A
//! hint carries grid coordinates, which the view can compare with the pointer
//! cell and with a selection.

mod path;
mod url;

use crate::grid::GridPoint;
use crate::grid::GridRange;
use crate::snapshot::Snapshot;

/// What a hint points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HintKind {
    /// A URL with a scheme Alacritty recognizes, including `file:`.
    Url,
    /// A filesystem path, recognized by tTerm rather than by Alacritty.
    Path,
}

/// One URL or path in the visible frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    /// The cells the hint covers, in absolute grid coordinates.
    pub range: GridRange,
    /// What kind of hint it is.
    pub kind: HintKind,
    /// The matched text, without the punctuation that was trimmed.
    pub text: String,
}

impl Hint {
    /// Whether the cell at `point` is part of the hint.
    pub fn contains(&self, point: GridPoint) -> bool {
        self.range.contains(point)
    }
}

/// Finds every URL and path in the visible frame, in reading order.
pub fn find_hints(snapshot: &Snapshot) -> Vec<Hint> {
    let grid = GridText::from_snapshot(snapshot);
    let urls = url::matches(&grid.text);
    let mut hints = Vec::new();
    for (start, end) in &urls {
        if let Some(hint) = grid.hint(*start, *end, HintKind::Url) {
            hints.push(hint);
        }
    }
    for (start, end) in path::matches(&grid.text, &urls) {
        if let Some(hint) = grid.hint(start, end, HintKind::Path) {
            hints.push(hint);
        }
    }
    hints.sort_by_key(|hint| (hint.range.start.line, hint.range.start.column));
    hints
}

/// The hint under a cell, if there is one.
pub fn hint_at(snapshot: &Snapshot, point: GridPoint) -> Option<Hint> {
    find_hints(snapshot)
        .into_iter()
        .find(|hint| hint.contains(point))
}

/// The visible text and the grid point of every character.
///
/// Wrapped rows are joined without a separator, because a URL or path may
/// continue onto the next row. A hard line break is kept as `\n` with no grid
/// point, so a pattern cannot match across it and the mapping never points at
/// a column that does not exist.
struct GridText {
    text: String,
    /// For every character, its byte offset in `text`.
    offsets: Vec<usize>,
    /// For every character, the cell it belongs to; `None` for `\n`.
    points: Vec<Option<GridPoint>>,
}

impl GridText {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        let mut grid = Self {
            text: String::new(),
            offsets: Vec::new(),
            points: Vec::new(),
        };
        let rows = snapshot.rows();
        for (row_index, row) in rows.iter().enumerate() {
            let line = snapshot.grid_line(row_index);
            for (column, cell) in row.cells().iter().enumerate() {
                let point = line.map(|line| GridPoint::new(line, column));
                grid.push(cell.ch, point);
                if let Some(extra) = &cell.zerowidth {
                    for character in extra {
                        grid.push(*character, point);
                    }
                }
            }
            let wrapped = row
                .cells()
                .last()
                .is_some_and(|cell| cell.attributes.wrapline);
            if !wrapped && row_index + 1 < rows.len() {
                grid.push('\n', None);
            }
        }
        grid
    }

    fn push(&mut self, character: char, point: Option<GridPoint>) {
        self.offsets.push(self.text.len());
        self.points.push(point);
        self.text.push(character);
    }

    /// Builds a hint from character indices, if both ends are in the grid.
    fn hint(&self, start: usize, end: usize, kind: HintKind) -> Option<Hint> {
        if start >= end || end > self.points.len() {
            return None;
        }
        let start_point = self.points.get(start).copied().flatten()?;
        let end_point = self.points.get(end - 1).copied().flatten()?;
        let start_byte = *self.offsets.get(start)?;
        // `end` is exclusive; when the match ends at the last character of
        // the frame there is no offsets[end], so use the string's end.
        let end_byte = if end == self.offsets.len() {
            self.text.len()
        } else {
            *self.offsets.get(end)?
        };
        let text = self.text.get(start_byte..end_byte)?.to_string();
        Some(Hint {
            range: GridRange::new(start_point, end_point),
            kind,
            text,
        })
    }
}

#[cfg(test)]
mod tests;
