//! ECMAScript string behaviour the ported frontend modules rely on.
//!
//! JavaScript strings are sequences of UTF-16 code units, and its whitespace
//! set differs from Rust's, so the ports that trim, split or measure text use
//! these helpers rather than the standard library's.

/// Matches JavaScript's `\s`: everything Rust calls whitespace, plus the
/// byte-order mark.
pub(crate) fn is_js_whitespace(character: char) -> bool {
    character.is_whitespace() || character == '\u{FEFF}'
}

/// Trims the characters JavaScript's `String.prototype.trim` removes.
pub(crate) fn trim_js(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// The length JavaScript reports for `value`, in UTF-16 code units.
pub(crate) fn utf16_len(value: &str) -> usize {
    value.chars().map(char::len_utf16).sum()
}

/// Returns what `value.slice(start, end)` would return.
///
/// A range that splits a surrogate pair cannot be represented in Rust, so the
/// half it covers becomes U+FFFD, as the theme catalog port already does.
pub(crate) fn utf16_slice(value: &str, start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    let mut units = 0;
    let mut result = String::new();
    for character in value.chars() {
        let length = character.len_utf16();
        let from = units;
        let to = units + length;
        units = to;
        if to <= start {
            continue;
        }
        if from >= end {
            break;
        }
        if from >= start && to <= end {
            result.push(character);
        } else {
            result.push('\u{FFFD}');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{trim_js, utf16_len, utf16_slice};

    #[test]
    fn trims_what_javascript_trims() {
        assert_eq!(trim_js("\u{FEFF} x \u{FEFF}"), "x");
        assert_eq!(trim_js("\u{2003}\tx\n"), "x");
    }

    #[test]
    fn measures_and_slices_utf16_code_units() {
        assert_eq!(utf16_len("a😀b"), 4);
        assert_eq!(utf16_slice("a😀b", 1, 3), "😀");
        assert_eq!(utf16_slice("a😀b", 2, 3), "\u{FFFD}");
        assert_eq!(utf16_slice("a😀b", 3, 5), "b");
        assert_eq!(utf16_slice("abc", 2, 2), "");
    }
}
