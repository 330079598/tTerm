//! The window title: the translated product title and a program's own title.

use gpui_kit::{App, Window};
use tterm_core::i18n::keys;

use super::{DEFAULT_TITLE, Prototype};

impl Prototype {
    /// The window title for a tab that has not set its own.
    pub(super) fn default_window_title(&self, cx: &App) -> String {
        let locale = self.ui.locale.read(cx);
        let title = match locale.text(keys::APP_TITLE) {
            Ok(title) => title,
            Err(error) => {
                eprintln!("tterm-gpui: failed to translate the window title: {error}");
                return DEFAULT_TITLE.to_string();
            }
        };
        let subtitle = match locale.text(keys::APP_SUBTITLE) {
            Ok(subtitle) => subtitle,
            Err(error) => {
                eprintln!("tterm-gpui: failed to translate the window subtitle: {error}");
                return title;
            }
        };
        format!("{title} — {subtitle}")
    }

    /// Applies `title` to the platform window when it changed.
    pub(super) fn apply_window_title(&mut self, window: &mut Window, title: String) {
        if self.current_title != title {
            window.set_window_title(&title);
            self.current_title = title;
        }
    }
}
