// Derived from Alacritty's `alacritty/src/config/ui_config.rs` (`URL_REGEX`)
// and `alacritty/src/display/hint.rs` (`HintPostProcessor`), commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the expression runs on the joined visible text instead of the
// grid iterator, and the post-processing works on string offsets instead of
// terminal points. The recognized schemes, the excluded characters and the
// trailing-punctuation rules are Alacritty's; the `{-}` range is kept because
// it excludes `{`, `|` and `}`.

//! URL recognition.
//!
//! Alacritty's default hint is a URL regular expression over the visible
//! viewport. This module keeps the same schemes and the same post-processing:
//! an unmatched closing bracket ends the match, and trailing punctuation is
//! removed. The result is a range of character indices into the joined text,
//! which [`super::GridText`] maps back to cells.

use std::sync::OnceLock;

use regex::Regex;

/// The schemes Alacritty's default hint accepts.
///
/// This is `alacritty/src/config/ui_config.rs`'s `URL_REGEX`. The character
/// class after the scheme excludes control characters, angle brackets,
/// quotes, whitespace, the `{-}` range (`{`, `|` and `}`), `^`, the Powerline
/// angle brackets, backticks and backslashes.
const URL_PATTERN: &str = r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file:|git://|ssh:|ftp://)[^\x00-\x1F\x7F-\x9F<>"\s{-}\^⟨⟩`\\]+"#;

/// The compiled expression, or `None` if it cannot be compiled.
///
/// A constant pattern cannot fail in practice, but the crate does not panic
/// in production code, so a failure simply finds no hints.
fn regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(URL_PATTERN).ok()).as_ref()
}

/// Finds every URL in `text`, as character-index ranges.
pub(crate) fn matches(text: &str) -> Vec<(usize, usize)> {
    let Some(regex) = regex() else {
        return Vec::new();
    };
    regex
        .find_iter(text)
        .filter_map(|found| {
            let (start, end) = trim(text, found.start(), found.end())?;
            Some((char_index(text, start), char_index(text, end)))
        })
        .collect()
}

/// Removes the characters Alacritty's post-processor removes.
///
/// An unmatched `)` or `]` ends the match, so `https://example.org/a(b)` is
/// kept whole while the closing bracket of `(https://example.org/a)` is not.
/// Trailing `.`, `,`, `:`, `;`, `?`, `!`, `(`, `[` and `'` are then trimmed.
pub(crate) fn trim(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let mut end = end;
    let mut open_parentheses = 0i32;
    let mut open_brackets = 0i32;
    for (offset, character) in text.get(start..end)?.char_indices() {
        match character {
            '(' => open_parentheses += 1,
            '[' => open_brackets += 1,
            ')' => {
                if open_parentheses == 0 {
                    end = start + offset;
                    break;
                }
                open_parentheses -= 1;
            }
            ']' => {
                if open_brackets == 0 {
                    end = start + offset;
                    break;
                }
                open_brackets -= 1;
            }
            _ => (),
        }
    }
    while let Some(last) = text.get(start..end)?.chars().next_back() {
        if matches!(last, '.' | ',' | ':' | ';' | '?' | '!' | '(' | '[' | '\'') {
            end -= last.len_utf8();
        } else {
            break;
        }
    }
    (end > start).then_some((start, end))
}

/// The character index of a byte offset in `text`.
fn char_index(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map(|prefix| prefix.chars().count())
        .unwrap_or(0)
}
