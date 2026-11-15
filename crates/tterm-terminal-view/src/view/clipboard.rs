//! Explicit terminal copy and paste gestures stay outside key encoding.

use std::rc::Rc;

use super::paste::{DropText, PasteRequest, paste_bytes};
use super::{PrimarySelection, TerminalView};
use gpui_kit::{App, ClipboardItem, Context, ExternalPaths, Keystroke, Window};
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::Scroll;

/// Reads the X11/Wayland primary selection on the platforms that have one.
///
/// Other platforms return `None`, so a Linux middle-click policy set in a
/// test cannot accidentally read the normal clipboard instead.
pub(super) fn platform_primary_selection() -> Option<PrimarySelection> {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        Some(Rc::new(|app: &gpui_kit::App| {
            app.read_from_primary().and_then(|item| item.text())
        }))
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        None
    }
}

impl TerminalView {
    pub(super) fn clipboard_key(
        &mut self,
        key: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
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
                    self.request_paste(&text, window, cx);
                }
                true
            }
            _ => false,
        }
    }

    /// Routes every paste gesture through the configured gate before any bytes
    /// reach the session.
    ///
    /// A gate returns `true` when it owns the request, even if it opens a
    /// confirmation dialog and sends nothing yet. Without a gate, the view
    /// keeps the old immediate paste behaviour for tests and failed windows.
    pub(super) fn request_paste(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            return;
        }
        let bracketed = self.terminal.modes().contains(Modes::BRACKETED_PASTE);
        if let Some(gate) = &self.paste_gate {
            let text = text.to_owned();
            let view = cx.entity().downgrade();
            let delivery = Rc::new(move |cx: &mut App, text: &str| {
                let _ = view.update(cx, |view, cx| {
                    view.paste_text(text, cx);
                });
            });
            if gate(
                PasteRequest {
                    text,
                    bracketed,
                    delivery,
                },
                window,
                cx,
            ) {
                return;
            }
        }
        self.paste_text(text, cx);
    }

    /// Sends text whose gate already approved it.
    ///
    /// The current bracketed mode is read here, not from the request, because
    /// a program can change the mode while a confirmation dialog is open.
    pub(super) fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let bytes = paste_bytes(text, self.terminal.modes().contains(Modes::BRACKETED_PASTE));
        if bytes.is_empty() {
            return;
        }
        self.terminal.clear_selection();
        self.terminal.scroll(Scroll::Bottom);
        self.send_bytes(&bytes, cx);
    }

    /// Copies a non-empty selection once, at the end of a local drag.
    pub(super) fn copy_selection_on_release(&mut self, cx: &mut Context<Self>) {
        if !self.copy_on_select {
            return;
        }
        let Some(text) = self.terminal.selection_text() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    /// Inserts dropped paths through the same paste gate, without Enter.
    pub(super) fn on_file_drop(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drop_text): Option<&DropText> = self.drop_text.as_ref() else {
            return;
        };
        let paths: Vec<String> = paths
            .paths()
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let text = drop_text(&paths);
        if !text.is_empty() {
            self.request_paste(&text, window, cx);
        }
    }
}
