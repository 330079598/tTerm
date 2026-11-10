//! Matching keyword rules in terminal text and mapping matches to cells.
use crate::{
    cell::Cell,
    color::{Color, NamedColor},
    snapshot::Snapshot,
};
use regex::{Regex, RegexBuilder};
use tterm_core::keyword_highlight::{HIGHLIGHT_PALETTE_COLORS, HighlightRule, HighlightSet};

/// Theme colors used to resolve named highlight colors.
pub type HighlightPalette = std::collections::HashMap<String, String>;
/// A compiled rule with resolved colors.
#[derive(Clone, Debug)]
pub struct CompiledHighlightRule {
    regex: Regex,
    pub foreground: Option<String>,
    pub background: Option<String>,
    whole_word: bool,
    strict_boundary: bool,
}
/// A match range measured in UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub foreground: Option<String>,
    pub background: Option<String>,
}
/// Maximum text inspected per line.
pub const MAX_HIGHLIGHT_TEXT: usize = 2000;
/// Maximum accepted spans per line.
pub const MAX_SPANS_PER_LINE: usize = 20;
const XTERM: [(&str, &str); 16] = [
    ("black", "#2e3436"),
    ("red", "#cc0000"),
    ("green", "#4e9a06"),
    ("yellow", "#c4a000"),
    ("blue", "#3465a4"),
    ("magenta", "#75507b"),
    ("cyan", "#06989a"),
    ("white", "#d3d7cf"),
    ("brightBlack", "#555753"),
    ("brightRed", "#ef2929"),
    ("brightGreen", "#8ae234"),
    ("brightYellow", "#fce94f"),
    ("brightBlue", "#729fcf"),
    ("brightMagenta", "#ad7fa8"),
    ("brightCyan", "#34e2e2"),
    ("brightWhite", "#eeeeec"),
];
fn escape(value: &str) -> String {
    regex::escape(value)
}
fn source(rule: &HighlightRule) -> String {
    if rule.id == "builtin-ipv6" {
        return r"(?:[\da-f]{1,4}:){1,7}[\da-f]{1,4}(?:/\d{1,3})?|[\da-f]{1,4}::[\da-f]{1,4}(?:/\d{1,3})?|::1".into();
    }
    if rule.kind == "keyword" {
        escape(&rule.pattern)
    } else if rule.id.starts_with("builtin-") {
        format!("(?:{})", rule.pattern)
            .replace(r"(?<![\w.])", "")
            .replace(r"(?<![\w:])", "")
            .replace(r"(?<![\w:-])", "")
            .replace(r"(?![\w]|\.\d)", "")
            .replace(r"(?![\w:])", "")
            .replace(r"(?![\w:-])", "")
            .replace(r"(?=[\da-f:]*\d)", "")
    } else {
        rule.pattern.clone()
    }
}
fn compile(rule: &HighlightRule) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&source(rule))
        .case_insensitive(!rule.case_sensitive)
        .build()
}
/// Why a pattern cannot be used.
pub fn highlight_pattern_error(rule: &HighlightRule) -> Option<&'static str> {
    if rule.pattern.trim().is_empty() {
        return Some("empty");
    }
    let Ok(regex) = compile(rule) else {
        return Some("invalid");
    };
    if regex.is_match("") {
        Some("matchesEmpty")
    } else {
        None
    }
}
/// Resolves a palette name or CSS hex color.
pub fn resolve_highlight_color(color: Option<&str>, palette: &HighlightPalette) -> Option<String> {
    let color = color?.trim();
    if HIGHLIGHT_PALETTE_COLORS.contains(&color) {
        return palette.get(color).cloned().or_else(|| {
            XTERM
                .iter()
                .find(|(name, _)| *name == color)
                .map(|(_, value)| (*value).into())
        });
    }
    tterm_terminal_color_to_hex(color)
}
fn tterm_terminal_color_to_hex(color: &str) -> Option<String> {
    crate::palette::Rgba::parse(color).map(crate::palette::Rgba::to_hex)
}
/// Compiles enabled, valid, colored rules in set order.
pub fn compile_highlight_set(
    set: &HighlightSet,
    palette: &HighlightPalette,
) -> Vec<CompiledHighlightRule> {
    set.rules
        .iter()
        .filter(|rule| rule.enabled && highlight_pattern_error(rule).is_none())
        .filter_map(|rule| {
            let foreground = resolve_highlight_color(rule.foreground.as_deref(), palette);
            let background = resolve_highlight_color(rule.background.as_deref(), palette);
            if foreground.is_none() && background.is_none() {
                return None;
            }
            Some(CompiledHighlightRule {
                regex: compile(rule).ok()?,
                foreground,
                background,
                whole_word: rule.whole_word,
                strict_boundary: rule.id.starts_with("builtin-ip") || rule.id == "builtin-mac",
            })
        })
        .collect()
}
fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}
fn utf16_offset(value: &str, byte: usize) -> usize {
    utf16_len(&value[..byte])
}
fn is_word(character: Option<char>) -> bool {
    character.is_some_and(|value| value.is_ascii_alphanumeric() || value == '_')
}
fn whole_word(text: &str, start: usize, end: usize) -> bool {
    !is_word(text[..start].chars().next_back()) && !is_word(text[end..].chars().next())
}
/// Finds non-overlapping spans, with earlier rules taking precedence.
pub fn match_highlights(text: &str, rules: &[CompiledHighlightRule]) -> Vec<HighlightSpan> {
    let input: String = text.chars().take(MAX_HIGHLIGHT_TEXT).collect();
    let mut accepted = Vec::new();
    for rule in rules {
        if accepted.len() >= MAX_SPANS_PER_LINE {
            break;
        }
        for matched in rule.regex.find_iter(&input) {
            if matched.start() == matched.end()
                || (rule.whole_word && !whole_word(&input, matched.start(), matched.end()))
                || (rule.strict_boundary
                    && !address_boundary(&input, matched.start(), matched.end(), &rule.regex))
            {
                continue;
            }
            let start = utf16_offset(&input, matched.start());
            let end = utf16_offset(&input, matched.end());
            if accepted
                .iter()
                .any(|span: &HighlightSpan| start < span.end && span.start < end)
            {
                continue;
            }
            accepted.push(HighlightSpan {
                start,
                end,
                foreground: rule.foreground.clone(),
                background: rule.background.clone(),
            });
            if accepted.len() >= MAX_SPANS_PER_LINE {
                break;
            }
        }
    }
    accepted.sort_by_key(|span| span.start);
    accepted
}

fn address_boundary(text: &str, start: usize, end: usize, regex: &Regex) -> bool {
    let matched = &text[start..end];
    let groups: Vec<&str> = matched.split(':').collect();
    if groups.len() == 3
        && groups.iter().all(|group| {
            !group.is_empty()
                && group.len() <= 2
                && group.chars().all(|character| character.is_ascii_digit())
        })
    {
        return false;
    }
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    let blocked = |value: Option<char>| {
        value.is_some_and(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-')
        })
    };
    if blocked(before) || blocked(after) {
        return false;
    }
    if regex.is_match(matched) && matched.contains(':') {
        return matched.chars().any(|character| character.is_ascii_digit());
    }
    true
}
/// Text and per-UTF-16-unit cell geometry read from terminal rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellText {
    pub text: String,
    pub rows: Vec<usize>,
    pub columns: Vec<usize>,
    pub widths: Vec<usize>,
    pub plain: Vec<bool>,
}
/// A rectangular row segment to color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightSegment {
    pub row: usize,
    pub x: usize,
    pub width: usize,
    pub foreground: Option<String>,
    pub background: Option<String>,
}
fn cell_plain(cell: &Cell) -> bool {
    matches!(cell.foreground, Color::Named(NamedColor::Foreground))
        && matches!(cell.background, Color::Named(NamedColor::Background))
        && !cell.attributes.inverse
}
fn cell_text(cell: &Cell) -> String {
    let mut text = String::new();
    if cell.ch != ' ' {
        text.push(cell.ch);
    }
    if let Some(extenders) = &cell.zerowidth {
        text.extend(extenders.iter().copied());
    }
    text
}
/// Reads visible rows as one wrapped text and removes trailing blank cells.
pub fn read_cell_text(
    snapshot: &Snapshot,
    start_row: usize,
    end_row: usize,
    max_length: usize,
) -> CellText {
    let mut result = CellText {
        text: String::new(),
        rows: Vec::new(),
        columns: Vec::new(),
        widths: Vec::new(),
        plain: Vec::new(),
    };
    let mut content_len = 0;
    for row_index in start_row..end_row.min(snapshot.line_count()) {
        let Some(row) = snapshot.row(row_index) else {
            break;
        };
        for (column, cell) in row.cells().iter().enumerate() {
            if result.text.encode_utf16().count() >= max_length {
                break;
            }
            if cell.attributes.wide_spacer {
                continue;
            }
            let chars = cell_text(cell);
            let units = if chars.is_empty() {
                " ".to_string()
            } else {
                chars
            };
            let width = if cell.attributes.wide { 2 } else { 1 };
            for _ in units.encode_utf16() {
                result.rows.push(row_index);
                result.columns.push(column);
                result.widths.push(width);
                result.plain.push(cell_plain(cell));
            }
            result.text.push_str(&units);
            if !cell.is_blank() {
                content_len = result.text.encode_utf16().count();
            }
        }
    }
    let mut truncate = result.text.len();
    let mut units = 0;
    for (index, character) in result.text.char_indices() {
        let next = units + character.len_utf16();
        if next > content_len {
            truncate = index;
            break;
        }
        units = next;
    }
    result.text.truncate(truncate);
    let length = result.text.encode_utf16().count();
    result.rows.truncate(length);
    result.columns.truncate(length);
    result.widths.truncate(length);
    result.plain.truncate(length);
    result
}
/// Maps UTF-16 spans to row segments, skipping cells already colored by the program.
pub fn highlight_segments(cells: &CellText, spans: &[HighlightSpan]) -> Vec<HighlightSegment> {
    let mut segments: Vec<HighlightSegment> = Vec::new();
    for span in spans {
        let mut current: Option<HighlightSegment> = None;
        for index in span.start..span.end.min(cells.text.encode_utf16().count()) {
            if !cells.plain.get(index).copied().unwrap_or(false) {
                current = None;
                continue;
            }
            let row = cells.rows[index];
            let x = cells.columns[index];
            let end = x + cells.widths[index];
            if let Some(segment) = current.as_ref()
                && segment.row == row
                && x <= segment.x + segment.width
            {
                if let Some(last) = segments.last_mut() {
                    last.width = last.width.max(end - last.x);
                }
                if let Some(current_segment) = current.as_mut() {
                    current_segment.width = current_segment.width.max(end - current_segment.x);
                }
                continue;
            }
            let segment = HighlightSegment {
                row,
                x,
                width: end - x,
                foreground: span.foreground.clone(),
                background: span.background.clone(),
            };
            segments.push(segment.clone());
            current = Some(segment);
        }
    }
    segments
}

#[cfg(test)]
mod tests;
