//! Model-side command marks for the overview ruler.
//!
//! The parser's OSC handler does not expose OSC 133 or 633, so a bounded
//! scanner finds those control strings and applies them at the exact byte
//! offset where Alacritty finished reading them. The tracker itself is the
//! existing [`CommandMarks`] state machine; this module only adapts the
//! model's grid to its viewport trait and caches the read-only line list.

use std::sync::Arc;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll as AnsiScroll;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::TermMode;

use crate::command_mark::CommandMark;
use crate::command_marks::{
    COMMAND_MARK_OSC_CODES, CommandMarkPosition, CommandMarkViewport, CommandMarks,
};
use crate::control_strings::ControlStringFramer;

/// The most payload bytes kept for one command mark.
const MAX_COMMAND_MARK_PAYLOAD_BYTES: usize = 4 * 1024;

/// A completed command mark and the byte offset just past its terminator.
pub(crate) struct CommandMarkEvent {
    pub(crate) end: usize,
    pub(crate) mark: CommandMark,
}

/// Finds OSC 133 and 633 payloads without becoming a second terminal parser.
pub(crate) struct CommandMarkScanner {
    framer: ControlStringFramer,
}

impl CommandMarkScanner {
    fn new() -> Self {
        const CODES: [u16; 2] = [
            COMMAND_MARK_OSC_CODES[0] as u16,
            COMMAND_MARK_OSC_CODES[1] as u16,
        ];
        Self {
            framer: ControlStringFramer::with_payload_codes(&CODES, MAX_COMMAND_MARK_PAYLOAD_BYTES),
        }
    }

    fn scan(&mut self, bytes: &[u8]) -> Vec<CommandMarkEvent> {
        let mut events = Vec::new();
        self.framer.scan(bytes, |event| {
            if !event.payload_valid || !matches!(event.code, Some(133 | 633)) {
                return;
            }
            let Ok(data) = std::str::from_utf8(event.payload) else {
                return;
            };
            if let Some(mark) = crate::command_marks::parse_command_mark(data) {
                events.push(CommandMarkEvent {
                    end: event.end,
                    mark,
                });
            }
        });
        events
    }
}

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
    scanner: CommandMarkScanner,
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
            scanner: CommandMarkScanner::new(),
            positions: Arc::from(Vec::new()),
        }
    }

    /// Finds the command marks completed by this write, without applying them.
    pub(crate) fn scan(&mut self, bytes: &[u8]) -> Vec<CommandMarkEvent> {
        self.scanner.scan(bytes)
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
