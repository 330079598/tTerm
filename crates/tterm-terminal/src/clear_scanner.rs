//! Detection of the scrollback-clearing screen erase.
//!
//! Alacritty clears the history on `CSI 3 J` but does not expose the event to
//! the model. A second, minimal parser watches the same bytes so the model can
//! release the row storage after the clear, including when the shell's prompt
//! follows in the same write. It uses the same VTE parser as Alacritty, so
//! split sequences and control strings behave exactly as they do in the grid.

use alacritty_terminal::vte::{Params, Parser, Perform};

/// Recognizes `CSI 3 J` across any number of writes.
#[derive(Default)]
pub(crate) struct ClearScanner {
    parser: Parser,
    saved_screen_cleared: bool,
}

impl ClearScanner {
    /// Scans `bytes` and reports whether they completed a saved-line erase.
    pub(crate) fn scan(&mut self, bytes: &[u8]) -> bool {
        self.saved_screen_cleared = false;
        // The parser is the handler's owner, so take it out while it calls
        // back into this scanner and put it back with its parse state intact.
        let mut parser = std::mem::take(&mut self.parser);
        parser.advance(self, bytes);
        self.parser = parser;
        std::mem::take(&mut self.saved_screen_cleared)
    }
}

impl Perform for ClearScanner {
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore || action != 'J' || !intermediates.is_empty() {
            return;
        }
        let Some(first) = params.iter().next() else {
            return;
        };
        // Alacritty uses the first parameter only; `0` is the default when
        // the sequence has no parameter at all.
        if first.first() == Some(&3) {
            self.saved_screen_cleared = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_ed_3_only() {
        assert!(ClearScanner::default().scan(b"\x1b[3J"));
        assert!(ClearScanner::default().scan(b"\x1b[03J"));
        assert!(!ClearScanner::default().scan(b"\x1b[2J"));
        assert!(!ClearScanner::default().scan(b"\x1b[1;3J"));
        assert!(!ClearScanner::default().scan(b"\x1b[?3J"));
        assert!(ClearScanner::default().scan(b"\x1b[3;1J"));
    }

    #[test]
    fn a_sequence_split_across_writes_is_still_one_clear() {
        let mut scanner = ClearScanner::default();
        assert!(!scanner.scan(b"\x1b["));
        assert!(!scanner.scan(b"3"));
        assert!(scanner.scan(b"J"));
        assert!(!scanner.scan(b""));
    }

    #[test]
    fn clear_text_inside_a_control_string_is_not_a_clear() {
        let mut scanner = ClearScanner::default();
        assert!(!scanner.scan(b"\x1b]0;a title 3J\x07"));
    }

    #[test]
    fn the_flag_is_reported_once_per_scan() {
        let mut scanner = ClearScanner::default();
        assert!(scanner.scan(b"\x1b[3J"));
        assert!(!scanner.scan(b"plain text"));
    }
}
