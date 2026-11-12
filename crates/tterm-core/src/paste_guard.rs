//! Rules for confirming a multi-line paste before it reaches the shell.

use tterm_terminal::js_text::{utf16_len, utf16_slice};

/// Lines of a guarded paste shown in the confirmation.
pub const PASTE_PREVIEW_MAX_LINES: usize = 8;
/// Characters kept of each previewed line.
pub const PASTE_PREVIEW_MAX_LINE_LENGTH: usize = 160;

/// Whether pasting `text` should be confirmed first.
///
/// A line break runs the line before it, unless the receiving program turned
/// on bracketed paste mode and so holds the whole paste until Enter.
pub fn paste_needs_confirmation(text: &str, enabled: bool, bracketed_paste_mode: bool) -> bool {
    enabled && !bracketed_paste_mode && (text.contains('\r') || text.contains('\n'))
}

/// The lines of a paste and the start of it to show before it is sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteSummary {
    /// The number of lines in the paste.
    pub line_count: usize,
    /// The preview shown in the confirmation.
    pub preview: String,
}

/// Summarizes a paste for the confirmation dialog.
pub fn summarize_paste(text: &str) -> PasteSummary {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }

    let mut shown: Vec<String> = lines
        .iter()
        .take(PASTE_PREVIEW_MAX_LINES)
        .map(|line| {
            if utf16_len(line) > PASTE_PREVIEW_MAX_LINE_LENGTH {
                format!("{}…", utf16_slice(line, 0, PASTE_PREVIEW_MAX_LINE_LENGTH))
            } else {
                (*line).to_owned()
            }
        })
        .collect();
    if lines.len() > PASTE_PREVIEW_MAX_LINES {
        shown.push("…".to_owned());
    }
    PasteSummary {
        line_count: lines.len(),
        preview: shown.join("\n"),
    }
}

#[cfg(test)]
mod tests;
