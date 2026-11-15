//! Clipboard paste normalization, independent of platform clipboard access.

use std::rc::Rc;

use gpui_kit::{App, Context, Window};

use super::TerminalView;

/// Sends already-approved clipboard text to the session input.
pub type PasteDelivery = Rc<dyn Fn(&mut App, &str)>;

/// One paste request waiting for the view's clipboard policy.
pub struct PasteRequest {
    /// The text as the clipboard, primary selection or drop supplied it.
    pub text: String,
    /// Whether bracketed paste mode was on when the request was made.
    pub bracketed: bool,
    /// Sends the request's text after the gate approves it.
    pub delivery: PasteDelivery,
}

/// Decides whether a paste may be sent now, may be confirmed first, or must
/// be rejected. Returning `false` lets the view deliver immediately.
pub type PasteGate = Rc<dyn Fn(PasteRequest, &mut Window, &mut Context<TerminalView>) -> bool>;

/// Quotes dropped paths for the local tab's shell.
pub type DropText = Rc<dyn Fn(&[String]) -> String>;

/// Encodes clipboard text without allowing an embedded bracket terminator.
///
/// Clipboard control characters are not keyboard gestures. Keep tabs and
/// carriage returns, remove the other C0/C1 controls, and normalize line
/// endings to CR before bracketing, as xterm.js does.
pub(super) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    let filtered: String = normalized
        .chars()
        .filter(|ch| *ch == '\r' || *ch == '\t' || !ch.is_control())
        .collect();
    if filtered.is_empty() {
        return Vec::new();
    }
    if bracketed {
        [
            b"\x1b[200~".as_slice(),
            filtered.as_bytes(),
            b"\x1b[201~".as_slice(),
        ]
        .concat()
    } else {
        filtered.into_bytes()
    }
}
