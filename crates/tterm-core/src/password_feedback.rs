//! The number of characters a no-echo password prompt holds.

/// How many characters a password prompt holds after `data` is typed.
///
/// The count follows a no-echo line reader: backspace erases, Ctrl+U clears,
/// Enter or Ctrl+C ends the prompt, and other control characters are ignored.
/// Iteration is by Unicode scalar value, as JavaScript's `for...of` does, so a
/// surrogate pair counts once.
pub fn next_typed_length(typed: i64, data: &str) -> Option<i64> {
    let mut length = typed;
    for character in data.chars() {
        match character {
            '\r' | '\n' | '\u{0003}' => return None,
            '\u{007F}' | '\u{0008}' => length = (length - 1).max(0),
            '\u{0015}' => length = 0,
            _ if character >= ' ' => length += 1,
            _ => {}
        }
    }
    Some(length)
}

#[cfg(test)]
mod tests;
