//! Captures the command line typed into a terminal from its input stream.

use std::sync::LazyLock;

use regress::Regex;

use crate::js_text::{
    from_utf16_units_lossy, js_trim, utf16_insert_at, utf16_len, utf16_remove_at,
    utf16_remove_before, utf16_slice, utf16_slice_from, utf16_units,
};

/// The line and cursor the capture has built so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCommandCaptureState {
    /// The captured line, in UTF-16 positions as the frontend counts them.
    pub text: String,
    /// The cursor position in UTF-16 code units.
    pub cursor: usize,
    /// Whether bracketed paste mode is in effect.
    pub bracketed_paste: bool,
    /// False once a key makes the captured line untrustworthy.
    pub valid: bool,
}

/// The state after a chunk and the commands it finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCommandCaptureResult {
    /// The state to pass to the next chunk.
    pub state: TerminalCommandCaptureState,
    /// The commands completed in this chunk.
    pub commands: Vec<String>,
}

/// The state before any input.
pub fn empty_command_capture_state() -> TerminalCommandCaptureState {
    TerminalCommandCaptureState {
        text: String::new(),
        cursor: 0,
        bracketed_paste: false,
        valid: true,
    }
}

static TRAILING_WORD: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\s*\S+\s*$").ok());

const BRACKETED_PASTE_START: &str = "\x1b[200~";
const BRACKETED_PASTE_END: &str = "\x1b[201~";

const EDIT_SEQUENCES: [&str; 9] = [
    "\x1b[1~", "\x1b[4~", "\x1b[3~", "\x1b[H", "\x1b[F", "\x1b[D", "\x1b[C", "\x1b[A", "\x1b[B",
];

/// Applies one input chunk to the capture state.
pub fn capture_terminal_input(
    current: &TerminalCommandCaptureState,
    data: &str,
) -> TerminalCommandCaptureResult {
    let mut state = current.clone();
    let mut commands = Vec::new();
    let mut byte_index = 0;

    while byte_index < data.len() {
        let rest = &data[byte_index..];
        if rest.starts_with(BRACKETED_PASTE_START) {
            state.bracketed_paste = true;
            byte_index += BRACKETED_PASTE_START.len();
            continue;
        }
        if rest.starts_with(BRACKETED_PASTE_END) {
            state.bracketed_paste = false;
            byte_index += BRACKETED_PASTE_END.len();
            continue;
        }

        if let Some(sequence) = EDIT_SEQUENCES
            .iter()
            .find(|candidate| rest.starts_with(**candidate))
        {
            apply_edit_sequence(&mut state, sequence);
            byte_index += sequence.len();
            continue;
        }

        let mut run = String::new();
        let mut run_bytes = 0;
        for (offset, character) in rest.char_indices() {
            // `\P{Cc}` excludes the C0 range and DEL; the pattern adds back
            // C1 (U+0080-U+009F), which is typed text here.
            let allowed = !matches!(character, '\u{0000}'..='\u{001F}' | '\u{007F}')
                || (state.bracketed_paste && matches!(character, '\r' | '\n'));
            if !allowed {
                break;
            }
            run.push(character);
            run_bytes = offset + character.len_utf8();
        }
        if !run.is_empty() {
            if state.bracketed_paste {
                run = run.replace('\r', "\n");
            }
            insert_at_cursor(&mut state, &run);
            byte_index += run_bytes;
            continue;
        }

        let Some(character) = rest.chars().next() else {
            break;
        };
        byte_index += character.len_utf8();
        match character {
            '\r' | '\n' => {
                if state.bracketed_paste {
                    insert_at_cursor(&mut state, "\n");
                } else {
                    let command = js_trim(&state.text).to_owned();
                    if state.valid && !command.is_empty() {
                        commands.push(command);
                    }
                    state.text.clear();
                    state.cursor = 0;
                    state.valid = true;
                }
            }
            '\u{007F}' | '\u{0008}' => {
                if state.cursor > 0 {
                    state.text = utf16_remove_before(&state.text, state.cursor);
                    state.cursor -= 1;
                }
            }
            '\u{0015}' | '\u{0003}' => {
                state.text.clear();
                state.cursor = 0;
                state.valid = true;
            }
            '\u{0017}' => {
                let before = utf16_slice(&state.text, 0, state.cursor);
                let stripped = strip_trailing_word(&before);
                let after = utf16_slice_from(&state.text, state.cursor);
                state.text = format!("{stripped}{after}");
                state.cursor = utf16_len(&stripped);
            }
            '\t' | '\u{001B}' => state.valid = false,
            _ if character >= ' ' => insert_at_cursor(&mut state, &character.to_string()),
            _ => {}
        }
    }

    TerminalCommandCaptureResult { state, commands }
}

fn apply_edit_sequence(state: &mut TerminalCommandCaptureState, sequence: &str) {
    match sequence {
        "\x1b[D" => state.cursor = state.cursor.saturating_sub(1),
        "\x1b[C" => state.cursor = (state.cursor + 1).min(utf16_len(&state.text)),
        "\x1b[H" | "\x1b[1~" => state.cursor = 0,
        "\x1b[F" | "\x1b[4~" => state.cursor = utf16_len(&state.text),
        "\x1b[3~" => {
            if state.cursor < utf16_len(&state.text) {
                state.text = utf16_remove_at(&state.text, state.cursor);
            }
        }
        "\x1b[A" | "\x1b[B" => {
            state.text.clear();
            state.cursor = 0;
            state.valid = false;
        }
        _ => {}
    }
}

fn insert_at_cursor(state: &mut TerminalCommandCaptureState, value: &str) {
    state.text = utf16_insert_at(&state.text, state.cursor, value);
    state.cursor += utf16_len(value);
}

fn strip_trailing_word(before: &str) -> String {
    let units = utf16_units(before);
    let Some(found) = TRAILING_WORD
        .as_ref()
        .and_then(|pattern| pattern.find_from_ucs2(&units, 0).next())
    else {
        return before.to_owned();
    };
    let range = found.range;
    format!(
        "{}{}",
        from_utf16_units_lossy(&units[..range.start]),
        from_utf16_units_lossy(&units[range.end..])
    )
}

/// The command line in an OSC 633 E payload, or in a C that carries one.
///
/// E escapes backslashes and control characters as `\\` and `\xHH`, as
/// tTerm's scripts and VS Code's shell integration send it.
pub fn parse_shell_integration_command(data: &str) -> Option<String> {
    let (marker, rest) = data.split_once(';')?;
    let command = if marker == "E" {
        unescape_command_line(rest)
    } else {
        rest.to_owned()
    };
    let command = js_trim(&command).to_owned();
    ((marker == "E" || marker == "C") && !command.is_empty()).then_some(command)
}

fn unescape_command_line(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '\\' {
            if characters.get(index + 1) == Some(&'\\') {
                output.push('\\');
                index += 2;
                continue;
            }
            if characters.get(index + 1) == Some(&'x') {
                let high = characters
                    .get(index + 2)
                    .and_then(|value| value.to_digit(16));
                let low = characters
                    .get(index + 3)
                    .and_then(|value| value.to_digit(16));
                if let (Some(high), Some(low)) = (high, low) {
                    if let Some(character) = char::from_u32(high * 16 + low) {
                        output.push(character);
                    }
                    index += 4;
                    continue;
                }
            }
        }
        output.push(characters[index]);
        index += 1;
    }
    output
}

#[cfg(test)]
mod tests;
