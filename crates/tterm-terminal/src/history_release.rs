//! Releases cached history rows when Alacritty executes a history clear.
//!
//! Alacritty's `ED 3` and full reset remove history rows from the grid's
//! logical length but keep up to 1000 trimmed rows in the storage cache. The
//! model needs to free that cache without changing any terminal semantics.
//!
//! Every byte the model parses goes through this handler, which forwards every
//! command to Alacritty unchanged. It acts only inside Alacritty's own
//! `clear_screen` and `reset_state` calls, so a truncation happens at exactly
//! the same point in the byte stream as the clear. That matters when the clear
//! shares a write with output that refills the history, with a switch to the
//! alternate screen, or with a synchronized update that delays execution.
//!
//! The forwarding list mirrors the `vte::ansi::Handler` trait from the pinned
//! `vte 0.15.0`. A new trait method with a default body would silently take
//! that default here, so the dependency pin must be rechecked when `vte` is
//! updated.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::cursor_icon::CursorIcon;
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink, KeyboardModes,
    KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, PrivateMode, Rgb,
    ScpCharPath, ScpUpdateMode, StandardCharset, TabulationClearMode,
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

/// Forwards parser commands to Alacritty and records main-screen history clears.
pub(crate) struct HistoryRelease<'a, T: EventListener> {
    term: &'a mut Term<T>,
    cleared_main: bool,
}

impl<'a, T: EventListener> HistoryRelease<'a, T> {
    /// Wraps a terminal for one parsing call.
    pub(crate) fn new(term: &'a mut Term<T>) -> Self {
        Self {
            term,
            cleared_main: false,
        }
    }

    /// Whether a command executed by this handler cleared the main history.
    pub(crate) fn cleared_main(&self) -> bool {
        self.cleared_main
    }
}

impl<T: EventListener> Handler for HistoryRelease<'_, T> {
    fn clear_screen(&mut self, mode: ClearMode) {
        let on_main = !self.term.mode().contains(TermMode::ALT_SCREEN);
        let saved = matches!(mode, ClearMode::Saved);
        self.term.clear_screen(mode);

        // `Saved` is the only clear mode that touches history. On the
        // alternate screen Alacritty clears the alternate grid, whose history
        // is empty, so truncating it cannot touch the hidden main history.
        if saved {
            self.term.grid_mut().truncate();
            self.cleared_main |= on_main;
        }
    }

    fn reset_state(&mut self) {
        self.term.reset_state();

        // A reset always leaves the main grid active: Alacritty swaps the
        // alternate grid out first. Both grids are reset, and the alternate
        // grid has no history, so truncating the active grid releases the main
        // history without touching what is left of the alternate screen.
        self.term.grid_mut().truncate();
        self.cleared_main = true;
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
        fn linefeed();
        fn bell();
        fn substitute();
        fn newline();
        fn set_horizontal_tabstop();
        fn scroll_up(a0: usize);
        fn scroll_down(a0: usize);
        fn insert_blank_lines(a0: usize);
        fn delete_lines(a0: usize);
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
        fn set_private_mode(a0: PrivateMode);
        fn unset_private_mode(a0: PrivateMode);
        fn report_private_mode(a0: PrivateMode);
        fn set_scrolling_region(a0: usize, a1: Option<usize>);
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
