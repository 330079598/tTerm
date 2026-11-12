//! The OSC 52 clipboard request the terminal answers.

use crate::js_encoding::decode_base64_utf8;

/// The OSC that programs use to set the clipboard (`ESC ] 52 ; Pc ; Pd BEL`).
pub const OSC52_CODE: u16 = 52;

/// Returns the text an OSC 52 sequence asks to copy.
///
/// `data` is the part after `52;`, so it has the form `Pc;Pd`. Every selection
/// target goes to the system clipboard, as iTerm2 and WezTerm do. A query
/// (`Pd` of `?`) and an empty payload both return `None`: neither may read or
/// clear the local clipboard on a program's request.
pub fn parse_osc52_copy(data: &str) -> Option<String> {
    let (targets, payload) = data.split_once(';')?;
    if !targets
        .chars()
        .all(|character| matches!(character, 'c' | 'p' | 'q' | 's' | '0'..='7'))
    {
        return None;
    }
    if payload.is_empty() || payload == "?" {
        return None;
    }
    decode_base64_utf8(payload).filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests;
