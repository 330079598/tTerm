//! Pure validation shared by the listener and its security regression tests.

use gpui_kit::{Keystroke, OperatingSystem};

pub(super) const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub(super) enum Command {
    Windows,
    State,
    Screenshot,
    Focus,
    Close,
    Key(Keystroke),
    Text(String),
}

/// Checks every token byte without an early return on a differing byte.
/// The expected token has a public, fixed length of 64 ASCII hex characters.
pub(super) fn token_matches(supplied: &[u8], expected: &[u8]) -> bool {
    let mut difference = supplied.len() ^ expected.len();
    for (index, expected) in expected.iter().enumerate() {
        difference = std::hint::black_box(
            difference | usize::from(supplied.get(index).copied().unwrap_or(0) ^ expected),
        );
    }
    std::hint::black_box(difference) == 0
}

pub(super) fn parse_request(
    input: &[u8],
    token: &str,
    os: OperatingSystem,
) -> Result<Command, &'static str> {
    if input.len() > MAX_REQUEST_BYTES {
        return Err("request exceeds 64 KiB");
    }
    let input = std::str::from_utf8(input).map_err(|_| "request is not UTF-8")?;
    let (auth, command) = input.split_once('\n').ok_or("missing command")?;
    let supplied = auth.strip_prefix("token=").unwrap_or_default();
    if !token_matches(supplied.as_bytes(), token.as_bytes()) {
        return Err("invalid token");
    }
    let command = command.strip_suffix('\n').unwrap_or(command);
    let command = command.strip_prefix("command=").ok_or("missing command")?;
    match command {
        "windows" => Ok(Command::Windows),
        "state" => Ok(Command::State),
        "screenshot" => Ok(Command::Screenshot),
        "action=focus" => Ok(Command::Focus),
        "action=close" => Ok(Command::Close),
        _ => {
            if let Some(key) = command.strip_prefix("key=") {
                return validate_key(key, os).map(Command::Key);
            }
            if let Some(text) = command.strip_prefix("text=") {
                validate_text(text)?;
                return Ok(Command::Text(text.to_string()));
            }
            Err("unknown command")
        }
    }
}

pub(super) fn validate_key(key: &str, os: OperatingSystem) -> Result<Keystroke, &'static str> {
    if key.is_empty() || key.len() > 64 || key.chars().any(char::is_control) {
        return Err("invalid keystroke");
    }
    // This is GPUI's dynamic entry point to the same parser used by parse().
    Keystroke::parse_with_os(key, os).map_err(|_| "invalid keystroke")
}

pub(super) fn validate_text(text: &str) -> Result<(), &'static str> {
    if text.len() > 4096 {
        return Err("text exceeds 4096 bytes");
    }
    if text.chars().any(char::is_control) {
        return Err("text contains control characters; use key=enter for a newline");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
