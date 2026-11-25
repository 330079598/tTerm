//! Live preferences shared by the parser and the view.

use super::ClipboardPolicy;
use super::Terminal;
use crate::snapshot::CursorShape;

impl Terminal {
    /// The configured scrollback capacity, in lines.
    pub fn scrollback(&self) -> usize {
        self.state.lock().config.scrollback
    }

    /// Changes the default without replacing an application's DECSCUSR override.
    pub fn set_default_cursor_shape(&self, shape: CursorShape) {
        let mut state = self.state.lock();
        if state.config.cursor_shape != shape {
            state.config.cursor_shape = shape;
            let config = state.config.to_alacritty();
            state.term.set_options(config);
        }
    }

    /// Lets programs store text with OSC 52, or stops accepting stores.
    ///
    /// Only the two write policies are reachable here: enabling maps to
    /// [`ClipboardPolicy::CopyOnly`], so a setting can never turn a query into
    /// a clipboard read. Disabling also drops the scanner's partial sequence,
    /// so a store that was being assembled cannot complete after a later
    /// enable. Alacritty's options do not change because both policies disable
    /// its own clipboard handling; the bounded scanner is the only write path.
    pub fn set_clipboard_write_enabled(&self, enabled: bool) {
        let mut state = self.state.lock();
        let policy = if enabled {
            ClipboardPolicy::CopyOnly
        } else {
            ClipboardPolicy::Disabled
        };
        if state.config.clipboard == policy {
            return;
        }
        state.config.clipboard = policy;
        state.osc52_scanner.set_enabled(enabled);
    }
}
