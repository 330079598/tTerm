//! Display-only import of xterm.js serialized history.
//!
//! The Tauri frontend saves a legacy tab as text, SGR runs, cursor moves,
//! erases and line feeds. Restoring it must not drive the live session: the
//! saved string comes from another process and may end in an incomplete
//! sequence. This module therefore parses into the Alacritty grid with a
//! throwaway processor and a handler that forwards only the commands
//! `SerializeAddon` writes: text, `CR`/`LF`, SGR, cursor motion and `ECH`.
//!
//! `SerializeAddon` 0.14 emits exactly `CSI n A`, `CSI n B`, `CSI n C`,
//! `CSI n D`, `CSI n X` and SGR `m` sequences around literal text. It does not
//! emit DECSC/DECRC, tab stops, modes, OSC or screen clears, so none of those
//! may reach the grid during an import.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::{Dimensions, Scroll as AnsiScroll};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Attr, ClearMode, Handler, Processor};

use super::{State, Terminal};
use crate::grid::GridSize;

/// Forwards the drawing subset of the ANSI handler to one Alacritty grid.
///
/// The allowed methods are exactly what `SerializeAddon` emits. Every other
/// method keeps the trait default, including queries, modes, OSC, DECSC/DECRC,
/// palette changes and `DECALN`. That keeps imported text from overwriting the
/// live session's saved cursor or any other persistent state.
struct LegacyHandler<'a, T: EventListener> {
    terminal: &'a mut Term<T>,
}

/// Generates a forwarding method for each allowed handler command.
///
/// The argument types are written out because a macro cannot read the
/// upstream trait.
macro_rules! forward {
    ($(fn $name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            fn $name(&mut self, $($arg: $ty),*) {
                Handler::$name(&mut *self.terminal, $($arg),*);
            }
        )*
    };
}

impl<T: EventListener> Handler for LegacyHandler<'_, T> {
    forward! {
        fn input(a0: char);
        fn move_up(a0: usize);
        fn move_down(a0: usize);
        fn move_forward(a0: usize);
        fn move_backward(a0: usize);
        fn carriage_return();
        fn linefeed();
        fn erase_chars(a0: usize);
        fn terminal_attribute(a0: Attr);
    }
}

/// Parses one byte slice into the grid with a fresh parser.
///
/// A fresh processor for every slice discards an incomplete control string at
/// the end of the slice instead of letting it consume the next slice.
fn parse_into<T: EventListener>(terminal: &mut Term<T>, bytes: &[u8]) {
    let mut processor: Processor = Processor::new();
    let mut handler = LegacyHandler { terminal };
    processor.advance(&mut handler, bytes);
}

/// Removes every control character from a banner before it is drawn.
fn banner_text(banner: &str) -> String {
    banner
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

impl Terminal {
    /// Restores legacy tab content for display without driving the session.
    ///
    /// Call this before the new session writes anything. Blank content is
    /// ignored as a whole, including its banner. Otherwise the saved history
    /// is drawn by a throwaway parser, the reset and sanitized banner are
    /// drawn by a second throwaway parser, and the viewport moves to the
    /// newest row. The live parser, scanners and intent queue are not
    /// touched.
    pub fn restore_legacy_history(&self, content: &str, banner: &str) {
        self.import_history(content, banner, false);
    }

    /// Imports above a clean viewport so absolute ConPTY redraws cannot overwrite history.
    pub fn restore_before_redraw(&self, content: &str, banner: &str) {
        self.import_history(content, banner, true);
    }

    fn import_history(&self, content: &str, banner: &str, fresh_viewport: bool) {
        if content.trim().is_empty() {
            return;
        }

        let banner = banner_text(banner);
        let tail = format!("\x1b[0m\r\n\x1b[2m── {banner} ──\x1b[0m\r\n");

        let mut state = self.state.lock();
        state.content_version = state.content_version.wrapping_add(1);
        // Inserted rows shift the absolute coordinates every cached history
        // row is keyed by, even when the bounded length does not change.
        state.scrollback_generation = state.scrollback_generation.wrapping_add(1);
        state.word_selection = false;
        state.word_drag = None;
        state.term.selection = None;
        state.command_marks.dispose_markers();

        state.absolute_redraw = fresh_viewport;
        let terminal = &mut state.term;
        parse_into(terminal, content.as_bytes());
        parse_into(terminal, tail.as_bytes());
        if fresh_viewport {
            terminal.clear_screen(ClearMode::All);
            terminal.goto(0, 0);
        }
        terminal.scroll_display(AnsiScroll::Bottom);
    }
}

impl State {
    /// Keeps ConPTY's origin fixed when a taller primary screen would pull history back into it.
    pub(super) fn resize_viewport(&mut self, size: GridSize) {
        let screen = self.term.screen_lines();
        if !self.absolute_redraw
            || size.screen_lines <= screen
            || self.term.mode().contains(TermMode::ALT_SCREEN)
        {
            self.term.resize(size);
            return;
        }
        self.term.resize(GridSize::new(size.columns, screen));
        let pulled = (size.screen_lines - screen).min(self.term.history_size());
        self.term.resize(size);
        if pulled != 0 {
            self.term.scroll_up(pulled);
            let grid = self.term.grid_mut();
            grid.cursor.point.line -= pulled;
            grid.saved_cursor.point.line -= pulled;
        }
    }
}

#[cfg(test)]
mod tests;
