//! Bare path recognition.
//!
//! Alacritty's hint only matches URLs and OSC 8 hyperlinks; it has no rule
//! for a path printed by a build tool. The rule below is tTerm's own and is
//! deliberately conservative, so it does not turn prose into links:
//!
//! - absolute POSIX paths (`/usr/bin`), home paths (`~/src`), explicit
//!   relative paths (`./run`, `../lib`) and UNC paths (`\\\\server\\share`);
//! - Windows drive paths (`C:\\Users\\me`);
//! - relative paths with a slash and a file extension in the last segment
//!   (`src/main.rs`).
//!
//! A token needs at least one separator and is trimmed of surrounding
//! quotes, brackets and trailing punctuation. A path inside a URL is not
//! reported twice: [`super::url::matches`] wins the overlap.

/// Finds every bare path in `text`, as character-index ranges.
///
/// `urls` are the URL ranges already found in the same text; paths that
/// overlap one are skipped.
pub(crate) fn matches(text: &str, urls: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut token_start: Option<usize> = None;
    for (offset, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = token_start.take() {
                consider(text, start, offset, urls, &mut results);
            }
        } else if token_start.is_none() {
            token_start = Some(offset);
        }
    }
    if let Some(start) = token_start {
        consider(text, start, text.len(), urls, &mut results);
    }
    results
}

/// Tests one whitespace-delimited token and adds it if it is a path.
fn consider(
    text: &str,
    start: usize,
    end: usize,
    urls: &[(usize, usize)],
    results: &mut Vec<(usize, usize)>,
) {
    let Some((start, end)) = trim_token(text, start, end) else {
        return;
    };
    let Some(token) = text.get(start..end) else {
        return;
    };
    if !looks_like_path(token) {
        return;
    }
    let start_char = char_index(text, start);
    let end_char = char_index(text, end);
    if urls
        .iter()
        .any(|(url_start, url_end)| start_char < *url_end && *url_start < end_char)
    {
        return;
    }
    results.push((start_char, end_char));
}

/// Removes surrounding punctuation from a token.
fn trim_token(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let mut start = start;
    let mut end = end;
    while let Some(character) = text.get(start..end)?.chars().next() {
        if matches!(character, '"' | '\'' | '(' | '[' | '{' | '<') {
            start += character.len_utf8();
        } else {
            break;
        }
    }
    while let Some(character) = text.get(start..end)?.chars().next_back() {
        if matches!(
            character,
            '"' | '\'' | ')' | ']' | '}' | '>' | '.' | ',' | ';' | ':' | '?' | '!'
        ) {
            end -= character.len_utf8();
        } else {
            break;
        }
    }
    (end > start).then_some((start, end))
}

/// Whether a trimmed token looks like a filesystem path.
fn looks_like_path(token: &str) -> bool {
    if token.contains("://") {
        return false;
    }
    if token.starts_with('/')
        || token.starts_with("~/")
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("\\\\")
    {
        return token.len() > 1;
    }
    let bytes = token.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }
    let separator = if token.contains('/') {
        '/'
    } else if token.contains('\\') {
        '\\'
    } else {
        return false;
    };
    let segments: Vec<&str> = token.split(separator).collect();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return false;
    }
    segments.last().is_some_and(|name| has_extension(name))
}

/// Whether the last segment has a short alphanumeric extension.
fn has_extension(name: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && !extension.is_empty()
        && extension.len() <= 8
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

/// The character index of a byte offset in `text`.
fn char_index(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map(|prefix| prefix.chars().count())
        .unwrap_or(0)
}
