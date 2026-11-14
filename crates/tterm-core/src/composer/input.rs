//! What the compose bar sends and suggests, from `src/lib/commandComposer.ts`.

use crate::js::{utf16_len, utf16_slice};

const PASTE_START: &str = "\u{1b}[200~";
const PASTE_END: &str = "\u{1b}[201~";

/// What to send for `text`, run at once: one line followed by Enter, or
/// several pasted the way a paste goes in, then Enter. Line breaks become CR,
/// as Enter types it. An empty text is a bare Enter.
pub fn composer_input(text: &str, bracketed_paste: bool) -> String {
    let lines = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = lines.trim_end_matches('\n').replace('\n', "\r");
    if !lines.contains('\r') || !bracketed_paste {
        return format!("{lines}\r");
    }
    // An end marker inside the text would end the paste early and run the rest.
    format!("{PASTE_START}{}{PASTE_END}\r", lines.replace(PASTE_END, ""))
}

/// The newest of `history` (newest first) that starts with `text` and goes on
/// past it, offered as `text` is typed, the way fish suggests; none for an
/// empty text or one of several lines.
pub fn history_suggestion(text: &str, history: &[String]) -> Option<String> {
    if text.is_empty() || text.contains('\n') {
        return None;
    }
    let length = utf16_len(text);
    history
        .iter()
        .find(|entry| utf16_len(entry) > length && entry.starts_with(text))
        .cloned()
}

/// No line break comes before the caret, so ArrowUp would not move it.
pub fn caret_on_first_line(text: &str, caret: usize) -> bool {
    !utf16_slice(text, 0, caret).contains('\n')
}

/// No line break comes after the caret, so ArrowDown would not move it.
pub fn caret_on_last_line(text: &str, caret: usize) -> bool {
    !utf16_slice(text, caret, utf16_len(text)).contains('\n')
}
