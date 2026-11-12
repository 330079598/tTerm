//! JavaScript string semantics over Rust's UTF-8 strings.
//!
//! JavaScript strings are sequences of UTF-16 code units. Lengths, indexes
//! and slices in the frontend count those units, so the ports that cut or
//! index text use these helpers rather than Rust's byte indexes.
//!
//! A Rust `String` cannot hold an unpaired surrogate. When a slice boundary
//! falls between the two halves of a surrogate pair, conversion replaces each
//! lost half with U+FFFD. The frontend can produce such a string, but no Rust
//! string can, so this is the closest representation available.

/// The length JavaScript reports for `text`, in UTF-16 code units.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The UTF-16 code units of `text`.
pub fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Builds a string from UTF-16 units, replacing unpaired surrogates.
pub fn from_utf16_units_lossy(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// JavaScript's `text.slice(start, end)` for code-unit positions.
pub fn utf16_slice(text: &str, start: usize, end: usize) -> String {
    let units = utf16_units(text);
    let start = start.min(units.len());
    let end = end.clamp(start, units.len());
    from_utf16_units_lossy(&units[start..end])
}

/// JavaScript's `text.slice(start)` for a code-unit position.
pub fn utf16_slice_from(text: &str, start: usize) -> String {
    utf16_slice(text, start, utf16_len(text))
}

/// The last `count` UTF-16 units of `text`, as JavaScript's `slice(-count)`.
pub fn utf16_tail(text: &str, count: usize) -> String {
    if count == 0 {
        return text.to_owned();
    }
    let length = utf16_len(text);
    utf16_slice(text, length.saturating_sub(count), length)
}

/// Inserts `value` at a UTF-16 code-unit position.
pub fn utf16_insert_at(text: &str, index: usize, value: &str) -> String {
    let mut units = utf16_units(text);
    let index = index.min(units.len());
    units.splice(index..index, utf16_units(value));
    from_utf16_units_lossy(&units)
}

/// Removes the UTF-16 unit at `index`, as one Delete key press does.
pub fn utf16_remove_at(text: &str, index: usize) -> String {
    let mut units = utf16_units(text);
    if index < units.len() {
        units.remove(index);
    }
    from_utf16_units_lossy(&units)
}

/// Removes the UTF-16 unit before `index`, as one Backspace press does.
pub fn utf16_remove_before(text: &str, index: usize) -> String {
    if index == 0 {
        return text.to_owned();
    }
    utf16_remove_at(text, index - 1)
}

/// Whether `character` is ECMAScript `\s`: WhiteSpace or a LineTerminator.
pub fn is_js_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// JavaScript's `String.prototype.trim`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_whitespace)
}

/// JavaScript's `String.prototype.trimEnd`.
pub fn js_trim_end(text: &str) -> &str {
    text.trim_end_matches(is_js_whitespace)
}

/// Whether `character` is in Unicode general category Cc.
pub fn is_control(character: char) -> bool {
    matches!(character, '\u{0000}'..='\u{001F}' | '\u{007F}'..='\u{009F}')
}

/// Whether `text` contains a character in Unicode general category Cc.
pub fn contains_control(text: &str) -> bool {
    text.chars().any(is_control)
}

#[cfg(test)]
mod tests;
