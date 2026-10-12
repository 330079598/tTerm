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
    /// When the output being parsed arrived; lines and events take their
    /// time from it, not from when the log got round to parsing them.
    now: DateTime<Local>,
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
            now: Local::now(),
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
            .unwrap_or(self.now);
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
        self.output.push(ScreenLine::Event { at: self.now, name });
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
            row.started = Some(self.now);
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
        let (last_at, now) = (self.last_at, self.now);
        *self = Screen::new(rows, cols);
        self.output = output;
        self.last_at = last_at;
        self.now = now;
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

    /// Parses output that arrived `at`.
    pub(super) fn advance(&mut self, at: DateTime<Local>, data: &[u8]) -> Vec<ScreenLine> {
        self.screen.now = at;
        self.parser.advance(&mut self.screen, data);
        std::mem::take(&mut self.screen.output)
    }

    pub(super) fn resize(&mut self, at: DateTime<Local>, rows: u16, cols: u16) -> Vec<ScreenLine> {
        self.screen.now = at;
        self.screen.resize(rows, cols);
        std::mem::take(&mut self.screen.output)
    }

    /// Logs everything still on screen; the session is over.
    pub(super) fn finish(&mut self, at: DateTime<Local>) -> Vec<ScreenLine> {
        self.screen.now = at;
        self.screen.finish();
        std::mem::take(&mut self.screen.output)
    }

    pub(super) fn oldest_pending(&self) -> Option<DateTime<Local>> {
        self.screen.oldest_pending()
    }
}

#[cfg(test)]
mod tests;
