//! The screen behind the plain-text log. Output is laid out on a grid the
//! way the terminal lays it out, and a line is logged once it scrolls off the
//! top of the main screen, so the log reads like the scrollback: redraws,
//! progress bars and prompts that a program rewrites in place leave only what
//! was finally on screen. Full-screen programs (vim, less, top) draw on the
//! alternate screen, which never reaches the scrollback; the log notes that
//! one ran instead of keeping its frames.

use chrono::{DateTime, Local};
use unicode_width::UnicodeWidthChar;
use vte::{Params, Perform};

const TAB_WIDTH: usize = 8;
/// Placeholder for the right half of a wide character.
const WIDE_TAIL: char = '\0';

pub(super) enum ScreenLine {
    /// A logical line of output (rows joined where they wrapped), stamped
    /// with when its first character appeared.
    Text { at: DateTime<Local>, text: String },
    /// Something the screen did that the log notes, such as a full-screen
    /// program starting.
    Event {
        at: DateTime<Local>,
        name: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Cell {
    ch: char,
    /// Zero-width characters (accents, joiners, variation selectors) that
    /// belong to this cell.
    combining: Option<Box<str>>,
}

impl Cell {
    const BLANK: Cell = Cell {
        ch: ' ',
        combining: None,
    };

    fn new(ch: char) -> Self {
        Self {
            ch,
            combining: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Row {
    /// Only as long as the rightmost cell written; the rest is blank.
    cells: Vec<Cell>,
    /// The line continues on the next row (the cursor wrapped off its end).
    wrapped: bool,
    started: Option<DateTime<Local>>,
}

impl Row {
    fn is_blank(&self) -> bool {
        self.cells
            .iter()
            .all(|cell| cell.ch == ' ' && cell.combining.is_none())
    }

    fn text(&self) -> String {
        let mut text = String::with_capacity(self.cells.len());
        for cell in &self.cells {
            if cell.ch != WIDE_TAIL {
                text.push(cell.ch);
            }
            if let Some(combining) = &cell.combining {
                text.push_str(combining);
            }
        }
        text
    }

    /// Writes `ch` at `col`, taking `col + 1` as well when it is wide.
    fn put(&mut self, col: usize, ch: char, wide: bool) {
        let end = col + usize::from(wide);
        if self.cells.len() <= end {
            self.cells.resize(end + 1, Cell::BLANK);
        }
        // Overwriting half of a wide character leaves no stray half behind.
        if self.cells[col].ch == WIDE_TAIL && col > 0 {
            self.cells[col - 1] = Cell::BLANK;
        }
        if self
            .cells
            .get(end + 1)
            .is_some_and(|next| next.ch == WIDE_TAIL)
        {
            self.cells[end + 1] = Cell::BLANK;
        }
        self.cells[col] = Cell::new(ch);
        if wide {
            self.cells[end] = Cell::new(WIDE_TAIL);
        }
    }

    fn blank_range(&mut self, start: usize, end: usize) {
        let end = end.min(self.cells.len());
        for cell in self.cells.iter_mut().take(end).skip(start) {
            *cell = Cell::BLANK;
        }
        self.settle();
    }

    fn truncate(&mut self, col: usize) {
        self.cells.truncate(col);
        self.wrapped = false;
        self.settle();
    }

    fn clear(&mut self) {
        *self = Row::default();
    }

    /// A row erased back to blank starts over: its next character stamps it.
    fn settle(&mut self) {
        if self.is_blank() {
            self.cells.clear();
            self.started = None;
        }
    }
}

struct SavedPrimary {
    grid: Vec<Row>,
    row: usize,
    col: usize,
    top: usize,
    bottom: usize,
}

struct Screen {
    rows: usize,
    cols: usize,
    grid: Vec<Row>,
    /// The main screen while a program has the alternate one.
    primary: Option<SavedPrimary>,
    row: usize,
    col: usize,
    /// The last column was written; the next character wraps first.
    pending_wrap: bool,
    top: usize,
    bottom: usize,
    saved_cursor: (usize, usize),
    auto_wrap: bool,
    origin_mode: bool,
    last_char: Option<char>,
    /// The start of a line whose rows have scrolled off while it is still
    /// wrapping onto rows that have not.
    logical: Option<(DateTime<Local>, String)>,
    last_at: Option<DateTime<Local>>,
    output: Vec<ScreenLine>,
}

impl Screen {
    fn new(rows: u16, cols: u16) -> Self {
        let rows = usize::from(rows).max(1);
        let cols = usize::from(cols).max(1);
        Self {
            rows,
            cols,
            grid: vec![Row::default(); rows],
            primary: None,
            row: 0,
            col: 0,
            pending_wrap: false,
            top: 0,
            bottom: rows - 1,
            saved_cursor: (0, 0),
            auto_wrap: true,
            origin_mode: false,
            last_char: None,
            logical: None,
            last_at: None,
            output: Vec::new(),
        }
    }

    fn in_alternate(&self) -> bool {
        self.primary.is_some()
    }

    fn commit(&mut self, row: Row) {
        let at = row
            .started
            .or_else(|| self.logical.as_ref().map(|(at, _)| *at))
            .or(self.last_at)
            .unwrap_or_else(Local::now);
        let wrapped = row.wrapped;
        let mut text = row.text();
        if let Some((started, mut head)) = self.logical.take() {
            head.push_str(&text);
            text = head;
            if wrapped {
                self.logical = Some((started, text));
                return;
            }
            self.emit_text(started, text);
            return;
        }
        if wrapped {
            self.logical = Some((at, text));
            return;
        }
        self.emit_text(at, text);
    }

    fn emit_text(&mut self, at: DateTime<Local>, text: String) {
        self.last_at = Some(at);
        self.output.push(ScreenLine::Text {
            at,
            text: text.trim_end().to_string(),
        });
    }

    fn flush_logical(&mut self) {
        if let Some((at, text)) = self.logical.take() {
            self.emit_text(at, text);
        }
    }

    fn event(&mut self, name: &'static str) {
        self.output.push(ScreenLine::Event {
            at: Local::now(),
            name,
        });
    }

    /// Logs what the main screen shows, up to its last non-blank row, and
    /// blanks it.
    fn commit_screen(&mut self) {
        let last = self.grid.iter().rposition(|row| !row.is_blank());
        if let Some(last) = last {
            let rows: Vec<Row> = self.grid[..=last].to_vec();
            for row in rows {
                self.commit(row);
            }
        }
        self.flush_logical();
        for row in &mut self.grid {
            row.clear();
        }
    }

    fn scroll_up(&mut self, count: usize) {
        for _ in 0..count.min(self.bottom - self.top + 1) {
            let removed = self.grid.remove(self.top);
            if self.top == 0 && !self.in_alternate() {
                self.commit(removed);
            }
            self.grid.insert(self.bottom, Row::default());
        }
    }

    fn scroll_down(&mut self, count: usize) {
        for _ in 0..count.min(self.bottom - self.top + 1) {
            self.grid.remove(self.bottom);
            self.grid.insert(self.top, Row::default());
        }
    }

    fn line_feed(&mut self) {
        self.pending_wrap = false;
        if self.row == self.bottom {
            self.scroll_up(1);
        } else if self.row + 1 < self.rows {
            self.row += 1;
        }
    }

    fn reverse_index(&mut self) {
        self.pending_wrap = false;
        if self.row == self.top {
            self.scroll_down(1);
        } else if self.row > 0 {
            self.row -= 1;
        }
    }

    fn wrap(&mut self) {
        self.grid[self.row].wrapped = true;
        self.line_feed();
        self.col = 0;
    }

    fn print_char(&mut self, ch: char) {
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            self.attach_combining(ch);
            return;
        }
        let width = if self.cols < 2 { 1 } else { width.min(2) };
        if self.pending_wrap {
            if self.auto_wrap {
                self.wrap();
            }
            self.pending_wrap = false;
        }
        if width == 2 && self.col + 1 >= self.cols {
            if self.auto_wrap {
                self.wrap();
            } else {
                self.col = self.cols - 2;
            }
        }
        let col = self.col;
        let row = &mut self.grid[self.row];
        row.put(col, ch, width == 2);
        if row.started.is_none() {
            row.started = Some(Local::now());
        }
        self.col += width;
        if self.col >= self.cols {
            self.col = self.cols - 1;
            self.pending_wrap = true;
        }
        self.last_char = Some(ch);
    }

    fn attach_combining(&mut self, ch: char) {
        let mut col = if self.pending_wrap {
            self.col
        } else if self.col > 0 {
            self.col - 1
        } else {
            return;
        };
        let row = &mut self.grid[self.row];
        if row.cells.get(col).is_some_and(|cell| cell.ch == WIDE_TAIL) && col > 0 {
            col -= 1;
        }
        if let Some(cell) = row.cells.get_mut(col) {
            let mut combining = cell.combining.take().map(String::from).unwrap_or_default();
            combining.push(ch);
            cell.combining = Some(combining.into_boxed_str());
        }
    }

    fn move_to(&mut self, row: usize, col: usize) {
        self.pending_wrap = false;
        self.row = row.min(self.rows - 1);
        self.col = col.min(self.cols - 1);
    }

    fn set_scroll_region(&mut self, top: usize, bottom: usize) {
        let bottom = bottom.min(self.rows - 1);
        if top >= bottom {
            return;
        }
        self.top = top;
        self.bottom = bottom;
        let home = if self.origin_mode { self.top } else { 0 };
        self.move_to(home, 0);
    }

    fn erase_display(&mut self, mode: u16) {
        match mode {
            0 => {
                let col = self.col;
                self.grid[self.row].truncate(col);
                for row in &mut self.grid[self.row + 1..] {
                    row.clear();
                }
            }
            1 => {
                let col = self.col;
                self.grid[self.row].blank_range(0, col + 1);
                for row in &mut self.grid[..self.row] {
                    row.clear();
                }
            }
            2 => {
                // A cleared screen never reaches the scrollback, but what it
                // showed is still output worth keeping (`clear` after `ls`).
                if self.in_alternate() {
                    for row in &mut self.grid {
                        row.clear();
                    }
                } else {
                    self.commit_screen();
                }
            }
            // ED 3 erases the scrollback, which the log has already kept.
            _ => {}
        }
    }

    fn erase_line(&mut self, mode: u16) {
        let col = self.col;
        let row = &mut self.grid[self.row];
        match mode {
            0 => row.truncate(col),
            1 => row.blank_range(0, col + 1),
            2 => row.clear(),
            _ => {}
        }
    }

    fn insert_lines(&mut self, count: usize) {
        if self.row < self.top || self.row > self.bottom {
            return;
        }
        for _ in 0..count.min(self.bottom - self.row + 1) {
            self.grid.remove(self.bottom);
            self.grid.insert(self.row, Row::default());
        }
        self.col = 0;
        self.pending_wrap = false;
    }

    fn delete_lines(&mut self, count: usize) {
        if self.row < self.top || self.row > self.bottom {
            return;
        }
        for _ in 0..count.min(self.bottom - self.row + 1) {
            self.grid.remove(self.row);
            self.grid.insert(self.bottom, Row::default());
        }
        self.col = 0;
        self.pending_wrap = false;
    }

    fn insert_chars(&mut self, count: usize) {
        let (col, cols) = (self.col, self.cols);
        let row = &mut self.grid[self.row];
        if col >= row.cells.len() {
            return;
        }
        for _ in 0..count.min(cols - col) {
            row.cells.insert(col, Cell::BLANK);
        }
        row.cells.truncate(cols);
        row.settle();
    }

    fn delete_chars(&mut self, count: usize) {
        let col = self.col;
        let row = &mut self.grid[self.row];
        if col >= row.cells.len() {
            return;
        }
        let end = (col + count).min(row.cells.len());
        row.cells.drain(col..end);
        row.settle();
    }

    fn erase_chars(&mut self, count: usize) {
        let col = self.col;
        self.grid[self.row].blank_range(col, col + count);
    }

    fn enter_alternate(&mut self, save_cursor: bool) {
        if self.in_alternate() {
            return;
        }
        if save_cursor {
            self.saved_cursor = (self.row, self.col);
        }
        let grid = std::mem::replace(&mut self.grid, vec![Row::default(); self.rows]);
        self.primary = Some(SavedPrimary {
            grid,
            row: self.row,
            col: self.col,
            top: self.top,
            bottom: self.bottom,
        });
        self.event("fullscreen_start");
    }

    fn leave_alternate(&mut self, restore_cursor: bool) {
        let Some(primary) = self.primary.take() else {
            return;
        };
        self.grid = primary.grid;
        self.top = primary.top;
        self.bottom = primary.bottom;
        let (row, col) = if restore_cursor {
            self.saved_cursor
        } else {
            (primary.row, primary.col)
        };
        self.move_to(row, col);
        self.event("fullscreen_end");
    }

    fn set_private_mode(&mut self, mode: u16, enabled: bool) {
        match mode {
            6 => {
                self.origin_mode = enabled;
                let home = if enabled { self.top } else { 0 };
                self.move_to(home, 0);
            }
            7 => self.auto_wrap = enabled,
            47 | 1047 => {
                if enabled {
                    self.enter_alternate(false);
                } else {
                    self.leave_alternate(false);
                }
            }
            1049 => {
                if enabled {
                    self.enter_alternate(true);
                } else {
                    self.leave_alternate(true);
                }
            }
            _ => {}
        }
    }

    fn reset(&mut self) {
        if self.in_alternate() {
            self.leave_alternate(false);
        }
        self.commit_screen();
        let (rows, cols) = (self.rows as u16, self.cols as u16);
        let output = std::mem::take(&mut self.output);
        let last_at = self.last_at;
        *self = Screen::new(rows, cols);
        self.output = output;
        self.last_at = last_at;
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        let rows = usize::from(rows).max(1);
        self.cols = usize::from(cols).max(1);
        if self.in_alternate() {
            // The main screen resizes underneath; its rows pushed off the top
            // reach the scrollback like any other.
            let mut primary = self.primary.take().expect("checked above");
            let committed = fit_rows(&mut primary.grid, &mut primary.row, rows);
            for row in committed {
                self.commit(row);
            }
            primary.top = 0;
            primary.bottom = rows - 1;
            primary.col = primary.col.min(self.cols - 1);
            self.primary = Some(primary);
            fit_rows(&mut self.grid, &mut self.row, rows);
        } else {
            for row in fit_rows(&mut self.grid, &mut self.row, rows) {
                self.commit(row);
            }
        }
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.move_to(self.row, self.col);
    }

    fn finish(&mut self) {
        if let Some(primary) = self.primary.take() {
            self.grid = primary.grid;
        }
        self.commit_screen();
    }

    /// When the earliest line still on screen started; nothing logged later
    /// can be older.
    fn oldest_pending(&self) -> Option<DateTime<Local>> {
        let grid = self
            .primary
            .as_ref()
            .map_or(&self.grid, |primary| &primary.grid);
        grid.iter()
            .filter_map(|row| row.started)
            .chain(self.logical.as_ref().map(|(at, _)| *at))
            .min()
    }
}

/// Grows or shrinks `grid` to `rows` the way the terminal does: blank rows
/// below the cursor go first, then rows leave from the top. Returns the rows
/// that left from the top.
fn fit_rows(grid: &mut Vec<Row>, cursor_row: &mut usize, rows: usize) -> Vec<Row> {
    let mut removed = Vec::new();
    while grid.len() > rows {
        let last = grid.len() - 1;
        if last > *cursor_row && grid[last].is_blank() {
            grid.pop();
        } else {
            removed.push(grid.remove(0));
            *cursor_row = cursor_row.saturating_sub(1);
        }
    }
    grid.resize(rows, Row::default());
    removed
}

fn param(params: &Params, index: usize, default: u16) -> u16 {
    params
        .iter()
        .nth(index)
        .and_then(|values| values.first())
        .copied()
        .filter(|value| *value != 0)
        .unwrap_or(default)
}

impl Perform for Screen {
    fn print(&mut self, ch: char) {
        self.print_char(ch);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => {
                self.pending_wrap = false;
                self.col = self.col.saturating_sub(1);
            }
            b'\t' => {
                self.pending_wrap = false;
                self.col = ((self.col / TAB_WIDTH + 1) * TAB_WIDTH).min(self.cols - 1);
            }
            b'\n' | 0x0b | 0x0c => self.line_feed(),
            b'\r' => {
                self.pending_wrap = false;
                self.col = 0;
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        if !intermediates.is_empty() {
            return;
        }
        match byte {
            b'D' => self.line_feed(),
            b'E' => {
                self.line_feed();
                self.col = 0;
            }
            b'M' => self.reverse_index(),
            b'7' => self.saved_cursor = (self.row, self.col),
            b'8' => {
                let (row, col) = self.saved_cursor;
                self.move_to(row, col);
            }
            b'c' => self.reset(),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        if intermediates == b"?" {
            if matches!(action, 'h' | 'l') {
                for values in params.iter() {
                    if let Some(mode) = values.first() {
                        self.set_private_mode(*mode, action == 'h');
                    }
                }
            }
            return;
        }
        if intermediates == b"!" && action == 'p' {
            // DECSTR soft reset.
            self.top = 0;
            self.bottom = self.rows - 1;
            self.auto_wrap = true;
            self.origin_mode = false;
            return;
        }
        if !intermediates.is_empty() {
            return;
        }

        let n = usize::from(param(params, 0, 1));
        let origin = if self.origin_mode { self.top } else { 0 };
        match action {
            'A' => {
                let floor = if self.row >= self.top { self.top } else { 0 };
                self.move_to(self.row.saturating_sub(n).max(floor), self.col);
            }
            'B' | 'e' => {
                let ceiling = if self.row <= self.bottom {
                    self.bottom
                } else {
                    self.rows - 1
                };
                self.move_to((self.row + n).min(ceiling), self.col);
            }
            'C' | 'a' => self.move_to(self.row, self.col + n),
            'D' => self.move_to(self.row, self.col.saturating_sub(n)),
            'E' => {
                let ceiling = if self.row <= self.bottom {
                    self.bottom
                } else {
                    self.rows - 1
                };
                self.move_to((self.row + n).min(ceiling), 0);
            }
            'F' => {
                let floor = if self.row >= self.top { self.top } else { 0 };
                self.move_to(self.row.saturating_sub(n).max(floor), 0);
            }
            'G' | '`' => self.move_to(self.row, n - 1),
            'd' => self.move_to(origin + n - 1, self.col),
            'H' | 'f' => {
                let col = usize::from(param(params, 1, 1));
                self.move_to(origin + n - 1, col - 1);
            }
            'J' => self.erase_display(param(params, 0, 0)),
            'K' => self.erase_line(param(params, 0, 0)),
            'L' => self.insert_lines(n),
            'M' => self.delete_lines(n),
            '@' => self.insert_chars(n),
            'P' => self.delete_chars(n),
            'X' => self.erase_chars(n),
            'S' => self.scroll_up(n),
            // With more parameters, `T` is a mouse-tracking request.
            'T' if params.len() <= 1 => self.scroll_down(n),
            'b' => {
                if let Some(ch) = self.last_char {
                    for _ in 0..n.min(self.cols * self.rows) {
                        self.print_char(ch);
                    }
                }
            }
            'r' => {
                let top = usize::from(param(params, 0, 1)) - 1;
                let bottom = usize::from(param(params, 1, self.rows as u16)) - 1;
                self.set_scroll_region(top, bottom);
            }
            's' if params.is_empty() => self.saved_cursor = (self.row, self.col),
            'u' => {
                let (row, col) = self.saved_cursor;
                self.move_to(row, col);
            }
            _ => {}
        }
    }
}

/// The output side of a plain-text log: bytes in, logged lines out.
pub(super) struct OutputLog {
    parser: vte::Parser,
    screen: Screen,
}

impl OutputLog {
    pub(super) fn new(rows: u16, cols: u16) -> Self {
        Self {
            parser: vte::Parser::new(),
            screen: Screen::new(rows, cols),
        }
    }

    pub(super) fn advance(&mut self, data: &[u8]) -> Vec<ScreenLine> {
        self.parser.advance(&mut self.screen, data);
        std::mem::take(&mut self.screen.output)
    }

    pub(super) fn resize(&mut self, rows: u16, cols: u16) -> Vec<ScreenLine> {
        self.screen.resize(rows, cols);
        std::mem::take(&mut self.screen.output)
    }

    /// Logs everything still on screen; the session is over.
    pub(super) fn finish(&mut self) -> Vec<ScreenLine> {
        self.screen.finish();
        std::mem::take(&mut self.screen.output)
    }

    pub(super) fn oldest_pending(&self) -> Option<DateTime<Local>> {
        self.screen.oldest_pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(lines: Vec<ScreenLine>) -> Vec<String> {
        lines
            .into_iter()
            .map(|line| match line {
                ScreenLine::Text { text, .. } => text,
                ScreenLine::Event { name, .. } => format!("<{name}>"),
            })
            .collect()
    }

    fn run(rows: u16, cols: u16, data: &[u8]) -> Vec<String> {
        let mut log = OutputLog::new(rows, cols);
        let mut lines = log.advance(data);
        lines.extend(log.finish());
        texts(lines)
    }

    #[test]
    fn logs_lines_once_they_scroll_off_the_top() {
        let mut log = OutputLog::new(3, 20);
        assert!(log.advance(b"one\r\ntwo\r\nthree").is_empty());
        assert_eq!(texts(log.advance(b"\r\nfour")), vec!["one"]);
        assert_eq!(texts(log.finish()), vec!["two", "three", "four"]);
    }

    #[test]
    fn keeps_only_the_last_frame_of_a_progress_line() {
        let lines = run(5, 40, b"get 10%\rget 50%\rget 100%\r\ndone\r\n");
        assert_eq!(lines, vec!["get 100%", "done"]);
    }

    #[test]
    fn redraws_of_several_lines_leave_the_final_frame() {
        // Two progress rows redrawn in place by moving the cursor up.
        let data = b"a: 1\r\nb: 1\r\n\x1b[2Aa: 2\x1b[K\r\nb: 2\x1b[K\r\n\x1b[2Aa: 3\x1b[K\r\nb: 3\x1b[K\r\n";
        assert_eq!(run(10, 20, data), vec!["a: 3", "b: 3"]);
    }

    #[test]
    fn rejoins_lines_the_terminal_wrapped() {
        let lines = run(3, 4, b"abcdefghij\r\nnext\r\n");
        assert_eq!(lines, vec!["abcdefghij", "next"]);
    }

    #[test]
    fn wrapped_line_split_across_scroll_and_screen_stays_whole() {
        let mut log = OutputLog::new(2, 4);
        let mut lines = log.advance(b"abcdefgh\r\nxy");
        lines.extend(log.finish());
        assert_eq!(texts(lines), vec!["abcdefgh", "xy"]);
    }

    #[test]
    fn full_screen_programs_leave_a_note_instead_of_frames() {
        let data = b"$ vim\r\n\x1b[?1049h\x1b[H\x1b[2Jfile contents\x1b[5;1H~\x1b[?1049l$ ls\r\n";
        assert_eq!(
            run(10, 20, data),
            vec!["<fullscreen_start>", "<fullscreen_end>", "$ vim", "$ ls"]
        );
    }

    #[test]
    fn clearing_the_screen_keeps_what_it_showed() {
        let data = b"$ ls\r\na b c\r\n$ clear\r\n\x1b[H\x1b[2J\x1b[3J$ ";
        assert_eq!(run(10, 20, data), vec!["$ ls", "a b c", "$ clear", "$"]);
    }

    #[test]
    fn lines_scrolled_out_of_a_partial_region_are_not_logged() {
        // A status line at the top with a scroll region below it.
        let data = b"status\x1b[2;3r\x1b[2;1Ha\r\nb\r\nc\r\nd";
        assert_eq!(run(3, 10, data), vec!["status", "c", "d"]);
    }

    #[test]
    fn wide_characters_take_two_columns() {
        assert_eq!(run(3, 5, "中文ab\r\n".as_bytes()), vec!["中文ab"]);
        // A wide character that does not fit wraps whole.
        assert_eq!(run(3, 5, "abcd中\r\n".as_bytes()), vec!["abcd中"]);
        // Overwriting half of one leaves no stray half.
        assert_eq!(run(3, 10, "中\rx\r\n".as_bytes()), vec!["x"]);
    }

    #[test]
    fn combining_marks_stay_with_their_character() {
        assert_eq!(run(3, 10, "e\u{301}x".as_bytes()), vec!["e\u{301}x"]);
    }

    #[test]
    fn shrinking_pushes_top_rows_into_the_log() {
        let mut log = OutputLog::new(4, 10);
        assert!(log.advance(b"1\r\n2\r\n3\r\n4").is_empty());
        assert_eq!(texts(log.resize(2, 10)), vec!["1", "2"]);
        assert_eq!(texts(log.finish()), vec!["3", "4"]);
    }

    #[test]
    fn prompt_redraw_after_backspaces_is_logged_once() {
        let mut log = OutputLog::new(5, 40);
        assert!(log
            .advance(b"\r\x1b[0m\x1b[32m> \x1b[36m~\x1b[0m \x1b[K")
            .is_empty());
        assert!(log.advance(b"l").is_empty());
        assert!(log.advance(b"\x08ll").is_empty());
        assert!(log
            .advance(b"\x08\x08\x1b[32ml\x1b[32ml\x1b[39m")
            .is_empty());
        assert!(log.advance(b"\x1b[?1l\x1b>\r\r\n").is_empty());
        assert_eq!(texts(log.finish()), vec!["> ~ ll"]);
    }

    #[test]
    fn oldest_pending_tracks_rows_still_on_screen() {
        let mut log = OutputLog::new(2, 10);
        assert!(log.oldest_pending().is_none());
        log.advance(b"a");
        let first = log.oldest_pending().expect("row started");
        log.advance(b"\r\nb\r\nc");
        let after = log.oldest_pending().expect("rows on screen");
        assert!(after >= first);
        log.finish();
        assert!(log.oldest_pending().is_none());
    }
}
