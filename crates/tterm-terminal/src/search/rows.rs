//! Copying retained history rows out of Alacritty's grid.
//!
//! The model calls this while it holds the terminal lock, so every call is
//! bounded to [`crate::search::SEARCH_CHUNK_ROWS`]. The conversion to text and
//! UTF-16 happens here; the scan itself never touches the grid.

use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell as AnsiCell, Flags};

use crate::search::{RetainedRows, SearchRow, UnitPosition};

/// Copies up to `max_rows` rows starting at retained row `start`.
pub(crate) fn read_rows(
    grid: &Grid<AnsiCell>,
    generation: u64,
    content_version: u64,
    start: usize,
    max_rows: usize,
) -> RetainedRows {
    let total_lines = grid.total_lines();
    let history_size = grid.history_size();
    let count = if start >= total_lines {
        0
    } else {
        max_rows.min(total_lines - start)
    };
    let mut rows = Vec::with_capacity(count);
    for index in start..start.saturating_add(count) {
        let Some(row) = read_row(grid, index, history_size) else {
            break;
        };
        rows.push(row);
    }
    RetainedRows::new(generation, content_version, total_lines, start, rows)
}

/// Converts one row's cells into UTF-16 units and cell positions.
fn read_row(grid: &Grid<AnsiCell>, index: usize, history_size: usize) -> Option<SearchRow> {
    let retained = i64::try_from(index).ok()?;
    let history = i64::try_from(history_size).ok()?;
    let absolute = i32::try_from(retained.checked_sub(history)?).ok()?;
    let row = &grid[Line(absolute)];
    let columns = row.len();
    let wrapped = columns
        .checked_sub(1)
        .is_some_and(|last| row[Column(last)].flags.contains(Flags::WRAPLINE));
    let mut units = Vec::with_capacity(columns);
    let mut positions = Vec::with_capacity(columns);
    let mut content_end = 0;
    for column in 0..columns {
        let cell = &row[Column(column)];
        if cell
            .flags
            .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            continue;
        }
        let width = if cell.flags.contains(Flags::WIDE_CHAR) {
            2
        } else {
            1
        };
        let position = UnitPosition::new(column, width)?;
        push_char(&mut units, &mut positions, cell.c, position);
        for &character in cell.zerowidth().unwrap_or_default() {
            push_char(&mut units, &mut positions, character, position);
        }
        if !is_blank(cell) {
            content_end = units.len();
        }
    }
    // xterm trims the right side of the last row of a hard line. A wrapped row
    // keeps its padding because the line continues on the next row.
    if !wrapped {
        units.truncate(content_end);
        positions.truncate(content_end);
    }
    Some(SearchRow::new(retained, units, positions, wrapped))
}

fn push_char(
    units: &mut Vec<u16>,
    positions: &mut Vec<UnitPosition>,
    character: char,
    position: UnitPosition,
) {
    let mut buffer = [0u16; 2];
    for unit in character.encode_utf16(&mut buffer) {
        units.push(*unit);
        positions.push(position);
    }
}

fn is_blank(cell: &AnsiCell) -> bool {
    matches!(cell.c, ' ' | '\t') && cell.zerowidth().is_none_or(<[char]>::is_empty)
}
