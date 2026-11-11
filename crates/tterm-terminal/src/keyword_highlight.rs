//! Matching JavaScript-compatible keyword rules in terminal text and mapping matches to cells.
use crate::{
    cell::Cell,
    color::{Color, NamedColor},
    snapshot::Snapshot,
};
use regress::{Flags, Regex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Palette values keyed by the frontend's sixteen terminal color names.
pub type HighlightPalette = HashMap<String, String>;
/// One configured highlight rule.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct HighlightRule {
    pub id: String,
    pub enabled: bool,
    pub pattern: String,
    pub kind: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub foreground: Option<String>,
    pub background: Option<String>,
}
impl Default for HighlightRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            pattern: String::new(),
            kind: "keyword".into(),
            case_sensitive: false,
            whole_word: false,
            foreground: None,
            background: None,
        }
    }
}
/// A named set of highlight rules.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct HighlightSet {
    pub id: String,
    pub name: String,
    pub rules: Vec<HighlightRule>,
}
/// A compiled rule with resolved colors.
#[derive(Clone, Debug)]
pub struct CompiledHighlightRule {
    regex: Regex,
    pub foreground: Option<String>,
    pub background: Option<String>,
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
const PALETTE_NAMES: [&str; 16] = [
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "black",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
    "brightBlack",
];
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
const IPV4_PATTERN: &str = r"(?<![\w.])(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}(?:/(?:3[0-2]|[12]?\d))?(?![\w]|\.\d)";
const IPV6_PATTERN: &str = r"(?<![\w:])(?=[\da-f:]*\d)(?:(?:[\da-f]{1,4}:){7}[\da-f]{1,4}|(?:[\da-f]{1,4}:){1,7}:|(?:[\da-f]{1,4}:){1,6}:[\da-f]{1,4}|(?:[\da-f]{1,4}:){1,5}(?::[\da-f]{1,4}){1,2}|(?:[\da-f]{1,4}:){1,4}(?::[\da-f]{1,4}){1,3}|(?:[\da-f]{1,4}:){1,3}(?::[\da-f]{1,4}){1,4}|(?:[\da-f]{1,4}:){1,2}(?::[\da-f]{1,4}){1,5}|[\da-f]{1,4}:(?::[\da-f]{1,4}){1,6}|:(?::[\da-f]{1,4}){1,7})(?:/\d{1,3})?(?![\w:])";
const MAC_PATTERN: &str = r"(?<![\w:-])(?:[\da-f]{2}(?::[\da-f]{2}){5}|[\da-f]{2}(?:-[\da-f]{2}){5}|[\da-f]{4}\.[\da-f]{4}\.[\da-f]{4})(?![\w:-])";
fn builtin_rule(id: &str, pattern: &str, foreground: &str, whole_word: bool) -> HighlightRule {
    HighlightRule {
        id: id.into(),
        enabled: true,
        pattern: pattern.into(),
        kind: "regex".into(),
        case_sensitive: false,
        whole_word,
        foreground: Some(foreground.into()),
        background: None,
    }
}
/// Returns the built-in rules shipped with the frontend.
pub fn builtin_highlight_set() -> HighlightSet {
    HighlightSet {
        id: "builtin".into(),
        name: String::new(),
        rules: vec![
            builtin_rule(
                "builtin-errors",
                r"errors?|fail(?:ed|ure|s)?|fatal|denied|refused|panic(?:ked)?|critical",
                "red",
                true,
            ),
            builtin_rule(
                "builtin-warnings",
                r"warn(?:ing|ings)?|timeout|timed out|deprecated",
                "yellow",
                true,
            ),
            builtin_rule(
                "builtin-success",
                r"success(?:ful|fully)?|succeeded|passed",
                "green",
                true,
            ),
            builtin_rule("builtin-ipv4", IPV4_PATTERN, "cyan", false),
            builtin_rule("builtin-ipv6", IPV6_PATTERN, "cyan", false),
            builtin_rule("builtin-mac", MAC_PATTERN, "magenta", false),
        ],
    }
}
fn escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|c| match c {
            '.' | '*' | '+' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' => {
                vec!['\\', c]
            }
            _ => vec![c],
        })
        .collect()
}
fn source(rule: &HighlightRule) -> String {
    let body = if rule.kind == "keyword" {
        escape(&rule.pattern)
    } else {
        rule.pattern.clone()
    };
    if rule.whole_word {
        format!(r"(?<!\w)(?:{})(?!\w)", body)
    } else {
        body
    }
}
fn flags(rule: &HighlightRule) -> Flags {
    Flags::from(if rule.case_sensitive { "g" } else { "gi" })
}
/// Why a pattern cannot be used.
pub fn highlight_pattern_error(rule: &HighlightRule) -> Option<&'static str> {
    if rule.pattern.trim().is_empty() {
        return Some("empty");
    }
    let Ok(regex) = Regex::with_flags(&source(rule), flags(rule)) else {
        return Some("invalid");
    };
    if regex.find_from_ucs2(&[], 0).next().is_some() {
        Some("matchesEmpty")
    } else {
        None
    }
}
fn parse_color(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let expanded = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 => hex.to_string(),
            _ => return None,
        };
        if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        return Some(format!("#{}", expanded.to_ascii_lowercase()));
    }
    let lower = value.to_ascii_lowercase();
    let body = if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        value[value.find('(')? + 1..].strip_suffix(')')?
    } else if lower.starts_with("hsl(") || lower.starts_with("hsla(") {
        return parse_hsl(value);
    } else {
        return None;
    };
    let parts: Vec<&str> = body
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|v| !v.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let channel = |v: &str| {
        v.parse::<f32>()
            .ok()
            .map(|n| n.clamp(0.0, 255.0).round() as u8)
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?
    ))
}
fn parse_hsl(value: &str) -> Option<String> {
    let body = value[value.find('(')? + 1..].strip_suffix(')')?;
    let parts: Vec<&str> = body
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|v| !v.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
    let s = parts[1]
        .trim_end_matches('%')
        .parse::<f32>()
        .ok()?
        .clamp(0.0, 100.0)
        / 100.0;
    let l = parts[2]
        .trim_end_matches('%')
        .parse::<f32>()
        .ok()?
        .clamp(0.0, 100.0)
        / 100.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 {
            t += 1.0
        }
        if t > 1.0 {
            t -= 1.0
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        (
            f(h / 360.0 + 1.0 / 3.0),
            f(h / 360.0),
            f(h / 360.0 - 1.0 / 3.0),
        )
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    ))
}
/// Resolves a palette value using the same CSS parser and xterm fallbacks as TS.
pub fn resolve_highlight_color(color: Option<&str>, palette: &HighlightPalette) -> Option<String> {
    let value = color?;
    if PALETTE_NAMES.contains(&value) {
        let supplied = palette.get(value).and_then(|v| parse_color(v));
        return supplied.or_else(|| {
            XTERM
                .iter()
                .find(|(name, _)| *name == value)
                .map(|(_, v)| (*v).into())
        });
    }
    parse_color(value)
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
                regex: Regex::with_flags(&source(rule), flags(rule)).ok()?,
                foreground,
                background,
            })
        })
        .collect()
}
/// Finds non-overlapping spans, with earlier rules taking precedence.
pub fn match_highlights(text: &str, rules: &[CompiledHighlightRule]) -> Vec<HighlightSpan> {
    let input: Vec<u16> = text.encode_utf16().take(MAX_HIGHLIGHT_TEXT).collect();
    let mut accepted = Vec::new();
    for rule in rules {
        if accepted.len() >= MAX_SPANS_PER_LINE {
            break;
        }
        for matched in rule.regex.find_from_ucs2(&input, 0) {
            if matched.start() == matched.end() {
                continue;
            }
            let start = matched.start();
            let end = matched.end();
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
    text.push(if cell.ch == '\t' { ' ' } else { cell.ch });
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
    let mut text_units = 0;
    for row_index in start_row..end_row.min(snapshot.line_count()) {
        let Some(row) = snapshot.row(row_index) else {
            break;
        };
        for (column, cell) in row.cells().iter().enumerate() {
            if text_units >= max_length {
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
            let added_units = units.encode_utf16().count();
            text_units += added_units;
            if !cell.is_blank() || cell.zerowidth.is_some() {
                content_len = text_units;
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
