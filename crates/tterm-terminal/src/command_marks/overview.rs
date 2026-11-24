//! Model-side command marks for the overview ruler.
//!
//! The parser's OSC handler does not expose OSC 133 or 633, so the model uses
//! the same bounded [`SessionReportScanner`] as the session layer and applies
//! a mark at the byte offset where Alacritty finished reading its control
//! string. The tracker itself is the existing [`CommandMarks`] state machine;
//! this module only adapts the model's grid to its viewport trait and caches
//! the read-only line list.

use std::sync::Arc;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll as AnsiScroll;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::TermMode;

use crate::command_marks::{CommandMarkPosition, CommandMarkViewport, CommandMarks};
use crate::session_reports::{CommandMarkEvent, SessionReportScanner};

/// The model's implementation of the tracker's viewport.
///
/// Lines are stored from the oldest retained history row, while Alacritty
/// counts visible rows from the top of the screen and keeps history negative.
/// Adding the current history size converts one coordinate to the other; a
/// screen scroll changes both terms together, so a stored line stays on the
/// same content.
struct ModelViewport<'a, T: EventListener> {
    term: &'a mut Term<T>,
}

impl<T: EventListener> ModelViewport<'_, T> {
    fn history_size(&self) -> i64 {
        self.term.history_size() as i64
    }
}

impl<T: EventListener> CommandMarkViewport for ModelViewport<'_, T> {
    fn is_normal_screen(&self) -> bool {
        !self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    fn cursor_line(&self) -> i64 {
        self.history_size() + self.term.grid().cursor.point.line.0 as i64
    }

    fn cursor_column(&self) -> usize {
        self.term.grid().cursor.point.column.0
    }

    fn viewport_line(&self) -> i64 {
        self.history_size() - self.term.grid().display_offset() as i64
    }

    fn base_line(&self) -> i64 {
        self.history_size()
    }

    fn scroll_to_line(&mut self, line: i64) {
        let history = self.history_size();
        let target = (history - line).clamp(0, history);
        let current = self.term.grid().display_offset() as i64;
        let delta = target - current;
        if delta != 0 {
            self.term.scroll_display(AnsiScroll::Delta(delta as i32));
        }
    }

    fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(AnsiScroll::Bottom);
    }
}

/// The tracker, its scanner and the cached read-only line list.
pub(crate) struct CommandMarkState {
    marks: CommandMarks,
    scanner: SessionReportScanner,
    positions: Arc<[CommandMarkPosition]>,
    positions_revision: u64,
}

impl CommandMarkState {
    /// Creates an empty tracker.
    pub(crate) fn new() -> Self {
        let marks = CommandMarks::new();
        Self {
            positions_revision: marks.revision(),
            marks,
            scanner: SessionReportScanner::for_command_marks(),
            positions: Arc::from(Vec::new()),
        }
    }

    /// Finds the command marks completed by this write, without applying them.
    pub(crate) fn scan(&mut self, bytes: &[u8]) -> Vec<CommandMarkEvent> {
        self.scanner.scan_command_marks(bytes)
    }

    /// Applies one mark after the parser has consumed its bytes.
    pub(crate) fn handle_event<T: EventListener>(
        &mut self,
        event: CommandMarkEvent,
        term: &mut Term<T>,
    ) {
        self.marks
            .handle_parsed_mark(event.mark, &mut ModelViewport { term });
        self.refresh();
    }

    /// Disposes every mark, as clearing the main history does.
    pub(crate) fn dispose_markers(&mut self) {
        self.marks.dispose_markers();
        self.refresh();
    }

    /// Moves marks after a known number of oldest history rows was removed.
    pub(crate) fn trim_scrollback(&mut self, lines: usize) {
        self.marks.trim_scrollback(lines);
        self.refresh();
    }

    /// The current read-only marks, cheaply shared with a frame.
    pub(crate) fn positions(&self) -> Arc<[CommandMarkPosition]> {
        Arc::clone(&self.positions)
    }

    fn refresh(&mut self) {
        let revision = self.marks.revision();
        if revision == self.positions_revision {
            return;
        }
        self.positions = self.marks.overview_marks().into();
        self.positions_revision = revision;
    }
}
