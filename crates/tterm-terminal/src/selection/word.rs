//! xterm's `_getWordAt` rules, expressed over the model's grid.
//!
//! Alacritty's semantic selection has two differences from the Tauri client:
//! it has its own separator set, and it runs `bracket_search` before the
//! separator scan. xterm does neither, so selecting a word by double click
//! is computed here and stored as a simple selection range.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::selection::Selection as AnsiSelection;
use alacritty_terminal::term::Term as AnsiTerm;
use alacritty_terminal::term::cell::{Cell as AnsiCell, Flags};

use crate::grid::GridPoint;
use crate::selection::{SelectionKind, SelectionRange, Side};

/// The word range at `point`, using xterm's separator rules.
///
/// A space expands over the contiguous run of spaces only. Every other
/// character, including punctuation that is itself a separator, expands over
/// the adjacent characters until another separator is reached. The range is
/// then widened across wrapped rows.
pub(crate) fn word_range<T: EventListener>(
    term: &AnsiTerm<T>,
    point: GridPoint,
    separators: &str,
) -> Option<SelectionRange> {
    let grid = term.grid();
    let columns = grid.columns();
    if columns == 0 {
        return None;
    }
    let top = grid.topmost_line().0;
    let bottom = grid.bottommost_line().0;
    let line = point.line.clamp(top, bottom);
    let mut column = point.column.min(columns - 1);
    column = lead_column(term, line, column);
    if cell(term, line, column).c == ' ' {
        let start = expand_spaces_left(term, line, column);
        let end = expand_spaces_right(term, line, column, columns);
        return Some(SelectionRange {
            start: GridPoint::new(line, start),
            end: GridPoint::new(line, end),
            block: false,
        });
    }

    let mut start_line = line;
    let mut start_column = expand_left(term, line, column, separators);
    let mut end_line = line;
    let mut end_column = expand_right(term, line, column, columns, separators);

    while start_column == 0 && start_line > top {
        let previous = start_line - 1;
        let last_char = line_end_character(term, previous, columns);
        let first = cell(term, start_line, 0);
        if is_separator(last_char, separators)
            || is_separator(first, separators)
            || !line_wraps(term, previous, columns)
        {
            break;
        }
        start_line = previous;
        start_column = expand_left(term, previous, columns - 1, separators);
    }
    while span_end(term, end_line, end_column, columns) == columns - 1 && end_line < bottom {
        let current_char = line_end_character(term, end_line, columns);
        let next = end_line + 1;
        let next_first = cell(term, next, 0);
        if !line_wraps(term, end_line, columns)
            || is_separator(current_char, separators)
            || is_separator(next_first, separators)
        {
            break;
        }
        end_line = next;
        end_column = expand_right(term, next, 0, columns, separators);
    }

    Some(SelectionRange {
        start: GridPoint::new(start_line, start_column),
        end: GridPoint::new(end_line, end_column),
        block: false,
    })
}

/// Writes a reading-order range as a simple Alacritty selection.
pub(crate) fn set_range<T: EventListener>(term: &mut AnsiTerm<T>, range: SelectionRange) {
    let mut selection = AnsiSelection::new(
        SelectionKind::Simple.into(),
        range.start.into(),
        Side::Left.into(),
    );
    selection.update(range.end.into(), Side::Right.into());
    term.selection = Some(selection);
}

fn cell<T: EventListener>(term: &AnsiTerm<T>, line: i32, column: usize) -> &AnsiCell {
    &term.grid()[Line(line)][Column(column)]
}

fn line_wraps<T: EventListener>(term: &AnsiTerm<T>, line: i32, columns: usize) -> bool {
    let last = cell(term, line, columns - 1);
    if last.flags.contains(Flags::WRAPLINE) {
        return true;
    }
    if is_spacer(last) && columns > 1 {
        return cell(term, line, columns - 2)
            .flags
            .contains(Flags::WRAPLINE);
    }
    false
}

fn line_end_character<T: EventListener>(
    term: &AnsiTerm<T>,
    line: i32,
    columns: usize,
) -> &AnsiCell {
    let last = cell(term, line, columns - 1);
    if is_spacer(last) && columns > 1 {
        let lead = cell(term, line, columns - 2);
        if !is_spacer(lead) {
            return lead;
        }
    }
    last
}

fn span_end<T: EventListener>(
    term: &AnsiTerm<T>,
    line: i32,
    column: usize,
    columns: usize,
) -> usize {
    if cell(term, line, column).flags.contains(Flags::WIDE_CHAR) && column + 1 < columns {
        column + 1
    } else {
        column
    }
}

fn is_spacer(cell: &AnsiCell) -> bool {
    cell.flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

fn is_separator(cell: &AnsiCell, separators: &str) -> bool {
    separators.contains(cell.c)
}

fn lead_column<T: EventListener>(term: &AnsiTerm<T>, line: i32, mut column: usize) -> usize {
    while column > 0 && is_spacer(cell(term, line, column)) {
        column -= 1;
    }
    column
}

fn expand_spaces_left<T: EventListener>(term: &AnsiTerm<T>, line: i32, mut column: usize) -> usize {
    while column > 0 && cell(term, line, column - 1).c == ' ' {
        column -= 1;
    }
    column
}

fn expand_spaces_right<T: EventListener>(
    term: &AnsiTerm<T>,
    line: i32,
    mut column: usize,
    columns: usize,
) -> usize {
    while column + 1 < columns && cell(term, line, column + 1).c == ' ' {
        column += 1;
    }
    column
}

fn expand_left<T: EventListener>(
    term: &AnsiTerm<T>,
    line: i32,
    mut column: usize,
    separators: &str,
) -> usize {
    while column > 0 {
        let mut previous = column - 1;
        if is_spacer(cell(term, line, previous)) {
            if previous == 0 {
                break;
            }
            previous -= 1;
        }
        if is_separator(cell(term, line, previous), separators) {
            break;
        }
        column = previous;
    }
    column
}

fn expand_right<T: EventListener>(
    term: &AnsiTerm<T>,
    line: i32,
    mut column: usize,
    columns: usize,
    separators: &str,
) -> usize {
    loop {
        let next = if cell(term, line, column).flags.contains(Flags::WIDE_CHAR) {
            column + 2
        } else {
            column + 1
        };
        if next >= columns || is_separator(cell(term, line, next), separators) {
            break;
        }
        column = next;
    }
    column
}
