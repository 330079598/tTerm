//! Live preferences shared by the parser and the view.

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
}
