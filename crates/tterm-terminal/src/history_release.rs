//! Releases cached history rows and records history movement while Alacritty
//! executes parser commands.
//!
//! Alacritty's `ED 3` and full reset remove history rows from the grid's
//! logical length but keep up to 1000 trimmed rows in the storage cache. The
//! model frees that cache without changing any terminal semantics.
//!
//! Every byte the model parses goes through this handler, which forwards every
//! command to Alacritty unchanged. It also observes the exact commands that
//! clear history or push its oldest rows out, so command marks move in the
//! same call as the rows they describe. That matters when a clear or scroll
//! shares a write with output that refills the history, with a switch to the
//! alternate screen, or with a synchronized update that delays execution.
//!
//! The forwarding list mirrors the `vte::ansi::Handler` trait from the pinned
//! `vte 0.15.0`. A new trait method with a default body would silently take
//! that default here, so the dependency pin must be rechecked when `vte` is
//! updated.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::{BidirectionalIterator, Dimensions, GridCell};
use alacritty_terminal::index::Point;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::cursor_icon::CursorIcon;
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink, KeyboardModes,
    KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, NamedPrivateMode,
    PrivateMode, Rgb, ScpCharPath, ScpUpdateMode, StandardCharset, TabulationClearMode,
};

/// Generates a forwarding method for each handler command.
///
/// The types are written out because a macro cannot read the upstream trait.
macro_rules! forward {
    ($(fn $name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            fn $name(&mut self, $($arg: $ty),*) {
                Handler::$name(&mut *self.term, $($arg),*);
            }
        )*
    };
}

/// The scrolling region Alacritty currently uses, in screen lines.
///
/// The region is not exposed by the upstream crate, so the wrapper mirrors the
/// same values when the parser sets it. A scroll only reaches history when the
/// region starts at the top of the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScrollRegion {
    start: i32,
    end: i32,
}

impl ScrollRegion {
    /// The full screen, which is the region before any `DECSTBM`.
    pub(crate) fn full(lines: usize) -> Self {
        Self {
            start: 0,
            end: i32::try_from(lines).unwrap_or(i32::MAX),
        }
    }

    fn contains(&self, line: i32) -> bool {
        line >= self.start && line < self.end
    }

    fn lines(&self) -> usize {
        usize::try_from(self.end.saturating_sub(self.start)).unwrap_or(0)
    }
}

/// How a parser command changed the marked history rows.
///
/// A dispose wins over any shifts in the same slice: the entries it removes
/// cannot be moved, and a later mark in the write starts a fresh entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MarkChanges {
    /// Remove every entry.
    pub(crate) dispose: bool,
    /// Drop this many oldest rows and shift the remaining marks.
    pub(crate) shift: usize,
}

impl MarkChanges {
    fn dispose(&mut self) {
        self.dispose = true;
        self.shift = 0;
    }

    fn shift(&mut self, lines: usize) {
        if !self.dispose {
            self.shift = self.shift.saturating_add(lines);
        }
    }
}

/// Forwards parser commands to Alacritty and records history changes.
///
/// The wrapper owns no parser state of its own except the mirrored scrolling
/// region. Every method forwards first and then records the exact history
/// change Alacritty just made, so a command mark applied at the end of a slice
/// sees the same grid state as the parser.
pub(crate) struct HistoryRelease<'a, T: EventListener> {
    term: &'a mut Term<T>,
    region: ScrollRegion,
    cleared_main: bool,
    marks: MarkChanges,
}

impl<'a, T: EventListener> HistoryRelease<'a, T> {
    /// Wraps a terminal for one parsing call with a full-screen region.
    #[cfg(test)]
    pub(crate) fn new(term: &'a mut Term<T>) -> Self {
        let region = ScrollRegion::full(term.screen_lines());
        Self::with_region(term, region)
    }

    /// Wraps a terminal and continues its mirrored scrolling region.
    pub(crate) fn with_region(term: &'a mut Term<T>, region: ScrollRegion) -> Self {
        Self {
            term,
            region,
            cleared_main: false,
            marks: MarkChanges::default(),
        }
    }

    /// Whether a command executed by this handler cleared the main history.
    pub(crate) fn cleared_main(&self) -> bool {
        self.cleared_main
    }

    /// The scrolling region after the parsed commands.
    pub(crate) fn region(&self) -> ScrollRegion {
        self.region
    }

    /// The marked-line changes made by the parsed commands.
    pub(crate) fn mark_changes(&self) -> MarkChanges {
        self.marks
    }

    fn is_normal_screen(&self) -> bool {
        !self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    fn history_size(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Records the oldest rows a successful scroll pushed out.
    fn record_shift(&mut self, positions: usize, history_before: usize) {
        if positions == 0 {
            return;
        }
        let added = self.history_size().saturating_sub(history_before);
        let evicted = positions.saturating_sub(added);
        if evicted > 0 {
            self.marks.shift(evicted);
        }
    }

    /// How many rows a linefeed at the cursor will scroll out of the top.
    fn linefeed_positions(&self) -> usize {
        let cursor = self.term.grid().cursor.point.line.0;
        if self.is_normal_screen() && self.region.start == 0 && cursor + 1 == self.region.end {
            1
        } else {
            0
        }
    }

    /// The rows `ED 2` will scroll up while it clears the viewport.
    fn clear_viewport_positions(&self) -> usize {
        let grid = self.term.grid();
        let mut iter = grid.iter_from(Point::new(grid.bottommost_line(), grid.last_column()));
        while let Some(cell) = iter.prev() {
            if !cell.is_empty() || cell.point.line < 0 {
                break;
            }
        }
        usize::try_from(iter.point().line.0 + 1).unwrap_or(0)
    }
}

impl<T: EventListener> Handler for HistoryRelease<'_, T> {
    fn clear_screen(&mut self, mode: ClearMode) {
        let on_main = self.is_normal_screen();
        let saved = matches!(mode, ClearMode::Saved);
        let all = matches!(mode, ClearMode::All);
        let positions = if on_main && all {
            self.clear_viewport_positions()
        } else {
            0
        };
        let history_before = self.history_size();
        Handler::clear_screen(&mut *self.term, mode);

        // `Saved` is the only clear mode that removes history. On the
        // alternate screen Alacritty clears the alternate grid, whose history
        // is empty, so truncating it cannot touch the hidden main history.
        if saved {
            self.term.grid_mut().truncate();
            self.cleared_main |= on_main;
            if on_main {
                self.marks.dispose();
            }
        } else if on_main {
            // `ED 2` moves the viewport's non-empty rows into history before
            // clearing it. That is a real scroll at capacity even though it is
            // not a clear of the saved lines.
            self.record_shift(positions, history_before);
        }
    }

    fn reset_state(&mut self) {
        Handler::reset_state(&mut *self.term);

        // A reset always leaves the main grid active: Alacritty swaps the
        // alternate grid out first. Both grids are reset, and the alternate
        // grid has no history, so truncating the active grid releases the main
        // history without touching what is left of the alternate screen.
        self.term.grid_mut().truncate();
        self.cleared_main = true;
        self.region = ScrollRegion::full(self.term.screen_lines());
        self.marks.dispose();
    }

    fn linefeed(&mut self) {
        let positions = self.linefeed_positions();
        let history_before = self.history_size();
        Handler::linefeed(&mut *self.term);
        self.record_shift(positions, history_before);
    }

    fn newline(&mut self) {
        let positions = self.linefeed_positions();
        let history_before = self.history_size();
        Handler::newline(&mut *self.term);
        self.record_shift(positions, history_before);
    }

    fn scroll_up(&mut self, lines: usize) {
        let positions = if self.is_normal_screen() && self.region.start == 0 {
            lines.min(self.region.lines())
        } else {
            0
        };
        let history_before = self.history_size();
        Handler::scroll_up(&mut *self.term, lines);
        self.record_shift(positions, history_before);
    }

    fn delete_lines(&mut self, lines: usize) {
        let origin = self.term.grid().cursor.point.line.0;
        let positions = if self.is_normal_screen()
            && origin == 0
            && lines > 0
            && self.region.start == 0
            && self.region.contains(origin)
        {
            lines.min(self.term.screen_lines()).min(self.region.lines())
        } else {
            0
        };
        let history_before = self.history_size();
        Handler::delete_lines(&mut *self.term, lines);
        self.record_shift(positions, history_before);
    }

    fn set_private_mode(&mut self, mode: PrivateMode) {
        let column_mode = matches!(mode, PrivateMode::Named(NamedPrivateMode::ColumnMode));
        Handler::set_private_mode(&mut *self.term, mode);
        if column_mode {
            // DECCOLM resets the scrolling region internally.
            self.region = ScrollRegion::full(self.term.screen_lines());
        }
    }

    fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
        Handler::set_scrolling_region(&mut *self.term, top, bottom);

        let screen_lines = self.term.screen_lines();
        let bottom = bottom.unwrap_or(screen_lines);
        if top < bottom {
            let start = i32::try_from(top).unwrap_or(i32::MAX) - 1;
            let end = i32::try_from(bottom).unwrap_or(i32::MAX);
            let screen = i32::try_from(screen_lines).unwrap_or(i32::MAX);
            self.region = ScrollRegion {
                start: start.min(screen),
                end: end.min(screen),
            };
        }
    }

    forward! {
        fn set_title(a0: Option<String>);
        fn set_cursor_style(a0: Option<CursorStyle>);
        fn set_cursor_shape(a0: CursorShape);
        fn input(a0: char);
        fn goto(a0: i32, a1: usize);
        fn goto_line(a0: i32);
        fn goto_col(a0: usize);
        fn insert_blank(a0: usize);
        fn move_up(a0: usize);
        fn move_down(a0: usize);
        fn identify_terminal(a0: Option<char>);
        fn device_status(a0: usize);
        fn move_forward(a0: usize);
        fn move_backward(a0: usize);
        fn move_down_and_cr(a0: usize);
        fn move_up_and_cr(a0: usize);
        fn put_tab(a0: u16);
        fn backspace();
        fn carriage_return();
        fn bell();
        fn substitute();
        fn set_horizontal_tabstop();
        fn scroll_down(a0: usize);
        fn insert_blank_lines(a0: usize);
        fn erase_chars(a0: usize);
        fn delete_chars(a0: usize);
        fn move_backward_tabs(a0: u16);
        fn move_forward_tabs(a0: u16);
        fn save_cursor_position();
        fn restore_cursor_position();
        fn clear_line(a0: LineClearMode);
        fn clear_tabs(a0: TabulationClearMode);
        fn set_tabs(a0: u16);
        fn reverse_index();
        fn terminal_attribute(a0: Attr);
        fn set_mode(a0: Mode);
        fn unset_mode(a0: Mode);
        fn report_mode(a0: Mode);
        fn unset_private_mode(a0: PrivateMode);
        fn report_private_mode(a0: PrivateMode);
        fn set_keypad_application_mode();
        fn unset_keypad_application_mode();
        fn set_active_charset(a0: CharsetIndex);
        fn configure_charset(a0: CharsetIndex, a1: StandardCharset);
        fn set_color(a0: usize, a1: Rgb);
        fn dynamic_color_sequence(a0: String, a1: usize, a2: &str);
        fn reset_color(a0: usize);
        fn clipboard_store(a0: u8, a1: &[u8]);
        fn clipboard_load(a0: u8, a1: &str);
        fn decaln();
        fn push_title();
        fn pop_title();
        fn text_area_size_pixels();
        fn text_area_size_chars();
        fn set_hyperlink(a0: Option<Hyperlink>);
        fn set_mouse_cursor_icon(a0: CursorIcon);
        fn report_keyboard_mode();
        fn push_keyboard_mode(a0: KeyboardModes);
        fn pop_keyboard_modes(a0: u16);
        fn set_keyboard_mode(a0: KeyboardModes, a1: KeyboardModesApplyBehavior);
        fn set_modify_other_keys(a0: ModifyOtherKeys);
        fn report_modify_other_keys();
        fn set_scp(a0: ScpCharPath, a1: ScpUpdateMode);
    }
}
#[cfg(test)]
mod tests {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::grid::Dimensions;
    use alacritty_terminal::term::test::TermSize;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::Processor;

    use super::*;

    fn model() -> Term<VoidListener> {
        Term::new(Config::default(), &TermSize::new(20, 6), VoidListener)
    }

    fn visible(terminal: &Term<VoidListener>) -> (Vec<(i32, usize, char)>, TermMode, usize, usize) {
        let content = terminal.renderable_content();
        let cells = content
            .display_iter
            .map(|indexed| (indexed.point.line.0, indexed.point.column.0, indexed.cell.c))
            .collect();
        (
            cells,
            *terminal.mode(),
            terminal.grid().display_offset(),
            terminal.total_lines(),
        )
    }

    /// A stream that reaches the parts of the handler a clear fix could break.
    ///
    /// The forwarding list is mechanical, so this is not an exhaustive check of
    /// every trait method. It is a guard against a wrapper that changes text,
    /// modes, cursor movement, colors, hyperlinks, synchronized updates or the
    /// alternate screen while releasing rows.
    const BROAD_STREAM: &[u8] = b"hello\r\n\
        \x1b[1;31mred\x1b[0m\r\n\
        \x1b[2;5Hxy\x1b[3A\x1b[2B\x1b[4C\x1b[2D\r\n\
        \x1b[2E\x1b[1F\x1b[3G\
        \x1b[2@\x1b[1P\x1b[2X\
        \x1b[1L\x1b[1M\r\n\
        \x1b[2S\x1b[1T\
        \x1b[1K\x1b[2K\
        \x1b[2;4r\x1b[4;1H\n\x1b[r\
        \x1b[?25l\x1b[?25h\x1b[?1h\x1b[?1l\
        \x1b]0;a title\x07\x1b]4;1;rgb:ff/00/00\x07\
        \x1b]8;;https://example.com\x07link\x1b]8;;\x07\
        \x1b[?1049halt\x1b[H\x1b[2J\x1b[3J\x1b[?1049l\
        \x1b[?2026hsynced\x1b[3J\x1b[?2026l\
        \x1bc\
        after reset\r\n";

    #[test]
    fn forwarding_preserves_the_grid_for_a_broad_stream() {
        let mut direct = model();
        let mut direct_parser: Processor = Processor::new();
        direct_parser.advance(&mut direct, BROAD_STREAM);

        let mut wrapped = model();
        let mut wrapped_parser: Processor = Processor::new();
        {
            let mut handler = HistoryRelease::new(&mut wrapped);
            wrapped_parser.advance(&mut handler, BROAD_STREAM);
        }

        assert_eq!(visible(&direct), visible(&wrapped));
    }
}
