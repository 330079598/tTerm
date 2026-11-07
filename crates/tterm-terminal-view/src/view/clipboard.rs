//! Explicit terminal copy and paste gestures stay outside key encoding.

use super::{TerminalView, paste::paste_bytes};
use gpui_kit::{ClipboardItem, Context, Keystroke};
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::Scroll;

impl TerminalView {
    pub(super) fn clipboard_key(&mut self, key: &Keystroke, cx: &mut Context<Self>) -> bool {
        let modifiers = key.modifiers;
        let primary = if cfg!(target_os = "macos") {
            modifiers.platform && !modifiers.control && !modifiers.alt
        } else {
            modifiers.control && modifiers.shift && !modifiers.alt && !modifiers.platform
        };
        if !primary {
            return false;
        }
        match key.key.as_str() {
            "c" => {
                if let Some(text) = self.terminal.selection_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                true
            }
            "v" => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.paste_text(&text, cx);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let bytes = paste_bytes(text, self.input_modes().contains(Modes::BRACKETED_PASTE));
        if bytes.is_empty() {
            return;
        }
        self.terminal.clear_selection();
        self.terminal.scroll(Scroll::Bottom);
        self.send_bytes(&bytes, cx);
    }
}
