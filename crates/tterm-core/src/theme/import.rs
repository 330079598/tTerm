//! Reading Ghostty, kitty, Alacritty, iTerm2 and Windows Terminal themes,
//! following `src/lib/themeImport.ts`.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use regress::Regex;
use serde::Serialize;
use serde_json::Value;

use tterm_terminal::theme::color_to_hex;
use tterm_terminal::theme::js::{js_number, js_round, js_trim};

/// A theme file format tTerm can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeFileFormat {
    /// Ghostty's `key = value` format.
    Ghostty,
    /// A Windows Terminal scheme or settings file.
    WindowsTerminal,
    /// An iTerm2 `.itermcolors` plist.
    Iterm,
    /// Alacritty's TOML or older YAML format.
    Alacritty,
    /// kitty's `key value` format.
    Kitty,
}

impl ThemeFileFormat {
    /// The string the TypeScript side uses.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ghostty => "ghostty",
            Self::WindowsTerminal => "windowsTerminal",
            Self::Iterm => "iterm",
            Self::Alacritty => "alacritty",
            Self::Kitty => "kitty",
        }
    }
}

/// One complete scheme from a theme file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedScheme {
    /// The scheme name, or the file name when the file has none.
    pub name: String,
    /// ANSI colors 0-15.
    pub palette: Vec<String>,
    /// The background color.
    pub background: String,
    /// The foreground color.
    pub foreground: String,
    /// The cursor color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Text drawn inside a block cursor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_text: Option<String>,
    /// The selection background.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_background: Option<String>,
    /// Selected text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_foreground: Option<String>,
}

/// The schemes a theme file contains.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeImport {
    /// The detected format.
    pub format: ThemeFileFormat,
    /// The complete schemes.
    pub schemes: Vec<ImportedScheme>,
}

/// Why a theme file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeImportError {
    /// The text does not look like a theme file.
    UnknownFormat,
    /// The background, foreground or one of the ANSI colors is missing.
    MissingColors(Vec<String>),
    /// A Windows Terminal file is not valid JSON.
    InvalidFile,
}

impl ThemeImportError {
    /// The TypeScript `reason` value.
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::UnknownFormat => "unknownFormat",
            Self::MissingColors(_) => "missingColors",
            Self::InvalidFile => "invalidFile",
        }
    }

    /// The names of the missing colors.
    pub fn missing(&self) -> &[String] {
        match self {
            Self::MissingColors(missing) => missing,
            Self::UnknownFormat | Self::InvalidFile => &[],
        }
    }
}

impl fmt::Display for ThemeImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat => formatter.write_str("unknownFormat"),
            Self::MissingColors(missing) => {
                write!(formatter, "Missing colors: {}", missing.join(", "))
            }
            Self::InvalidFile => formatter.write_str("invalidFile"),
        }
    }
}

impl std::error::Error for ThemeImportError {}

#[derive(Clone, Debug, Default)]
struct PartialScheme {
    name: Option<String>,
    background: Option<String>,
    foreground: Option<String>,
    palette: Vec<Option<String>>,
    cursor: Option<String>,
    cursor_text: Option<String>,
    selection_background: Option<String>,
    selection_foreground: Option<String>,
}

impl PartialScheme {
    fn set_palette(&mut self, index: usize, value: Option<String>) {
        if self.palette.len() <= index {
            self.palette.resize(index + 1, None);
        }
        self.palette[index] = value;
    }
}

const ANSI_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// The schemes in a theme file.
///
/// `file_name` names a scheme the file leaves unnamed; one empty string means
/// the TypeScript default parameter.
pub fn parse_theme_file(text: &str, file_name: &str) -> Result<ThemeImport, ThemeImportError> {
    let Some(format) = detect_format(text) else {
        return Err(ThemeImportError::UnknownFormat);
    };
    let fallback_name = fallback_name(file_name);

    if format == ThemeFileFormat::WindowsTerminal {
        let json = parse_jsonc(text).map_err(|()| ThemeImportError::InvalidFile)?;
        let schemes = read_windows_terminal(&json);
        let mut complete = Vec::new();
        for scheme in &schemes {
            if let Ok(scheme) = complete_scheme(scheme, &fallback_name) {
                complete.push(scheme);
            }
        }
        if complete.is_empty() {
            let first = schemes.first().cloned().unwrap_or_default();
            let scheme = complete_scheme(&first, &fallback_name)?;
            return Ok(ThemeImport {
                format,
                schemes: vec![scheme],
            });
        }
        return Ok(ThemeImport {
            format,
            schemes: complete,
        });
    }

    let partial = match format {
        ThemeFileFormat::Ghostty => read_ghostty(text),
        ThemeFileFormat::Kitty => read_kitty(text),
        ThemeFileFormat::Alacritty => read_alacritty(text),
        ThemeFileFormat::Iterm => read_iterm(text),
        ThemeFileFormat::WindowsTerminal => PartialScheme::default(),
    };
    Ok(ThemeImport {
        format,
        schemes: vec![complete_scheme(&partial, &fallback_name)?],
    })
}

fn fallback_name(file_name: &str) -> String {
    static EXTENSION: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"\.[^.]+$", "").ok());
    let without_extension = EXTENSION
        .as_ref()
        .map(|pattern| pattern.replace(file_name, ""))
        .unwrap_or_else(|| file_name.to_string());
    let trimmed = js_trim(&without_extension);
    if trimmed.is_empty() {
        "Imported theme".into()
    } else {
        trimmed.to_string()
    }
}

fn complete_scheme(
    scheme: &PartialScheme,
    fallback_name: &str,
) -> Result<ImportedScheme, ThemeImportError> {
    let mut missing = Vec::new();
    if scheme.background.as_deref().is_none_or(str::is_empty) {
        missing.push("background".to_string());
    }
    if scheme.foreground.as_deref().is_none_or(str::is_empty) {
        missing.push("foreground".to_string());
    }
    for index in 0..16 {
        if scheme
            .palette
            .get(index)
            .and_then(|value| value.as_deref())
            .is_none_or(str::is_empty)
        {
            missing.push(format!("color{index}"));
        }
    }
    if !missing.is_empty() {
        return Err(ThemeImportError::MissingColors(missing));
    }

    let name = scheme
        .name
        .as_deref()
        .map(js_trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name)
        .to_string();
    Ok(ImportedScheme {
        name,
        background: scheme.background.clone().unwrap_or_default(),
        foreground: scheme.foreground.clone().unwrap_or_default(),
        palette: (0..16)
            .map(|index| {
                scheme
                    .palette
                    .get(index)
                    .and_then(|value| value.clone())
                    .unwrap_or_default()
            })
            .collect(),
        cursor: scheme.cursor.clone(),
        cursor_text: scheme.cursor_text.clone(),
        selection_background: scheme.selection_background.clone(),
        selection_foreground: scheme.selection_foreground.clone(),
    })
}

fn parse_color(value: Option<&str>) -> Option<String> {
    let value = value.filter(|value| !value.is_empty())?;
    let mut trimmed = js_trim(value).chars().collect::<Vec<_>>();
    if matches!(trimmed.first(), Some('\'') | Some('"')) {
        trimmed.remove(0);
    }
    if matches!(trimmed.last(), Some('\'') | Some('"')) {
        trimmed.pop();
    }
    let trimmed = trimmed.into_iter().collect::<String>();

    let without_prefix = trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("0x"))
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(&trimmed);
    if (without_prefix.len() == 6 || without_prefix.len() == 8)
        && without_prefix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Some(format!("#{}", without_prefix[..6].to_ascii_lowercase()));
    }
    color_to_hex(&trimmed)
}

fn split_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let end = if index > start && bytes[index - 1] == b'\r' {
                index - 1
            } else {
                index
            };
            lines.push(&text[start..end]);
            start = index + 1;
        }
    }
    lines.push(&text[start..]);
    lines
}

fn read_ghostty(text: &str) -> PartialScheme {
    static LINE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*([\w-]+)\s*=\s*(.*?)\s*$", "").ok());
    static ENTRY: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^(\d+)\s*=\s*(\S+)$", "").ok());

    let mut scheme = PartialScheme::default();
    for line in split_lines(text) {
        let Some(captures) = LINE.as_ref().and_then(|pattern| pattern.find(line)) else {
            continue;
        };
        let key = captures
            .group(1)
            .map(|range| &line[range])
            .unwrap_or_default();
        let value = captures
            .group(2)
            .map(|range| &line[range])
            .unwrap_or_default();
        if key == "palette" {
            if let Some(entry) = ENTRY.as_ref().and_then(|pattern| pattern.find(value)) {
                let index = entry
                    .group(1)
                    .map(|range| js_number(&value[range]))
                    .unwrap_or(f64::NAN);
                let color = entry.group(2).map(|range| &value[range]);
                if index < 16.0 {
                    scheme.set_palette(index as usize, parse_color(color));
                }
            }
        } else if let Some(field) = ghostty_special_field(key) {
            set_special(&mut scheme, field, parse_color(Some(value)));
        }
    }
    scheme
}

fn read_kitty(text: &str) -> PartialScheme {
    static NAME: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*##\s*name\s*:\s*(.+?)\s*$", "im").ok());
    static LINE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*(\w+)\s+(\S+)\s*$", "").ok());
    static INDEX: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^color(\d+)$", "").ok());

    let name = NAME
        .as_ref()
        .and_then(|pattern| pattern.find(text))
        .and_then(|captures| captures.group(1))
        .map(|range| text[range].to_string());
    let mut scheme = PartialScheme {
        name,
        ..PartialScheme::default()
    };
    for line in split_lines(text) {
        let Some(captures) = LINE.as_ref().and_then(|pattern| pattern.find(line)) else {
            continue;
        };
        let key = captures
            .group(1)
            .map(|range| &line[range])
            .unwrap_or_default();
        let value = captures
            .group(2)
            .map(|range| &line[range])
            .unwrap_or_default();
        let index = INDEX
            .as_ref()
            .and_then(|pattern| pattern.find(key))
            .and_then(|captures| captures.group(1))
            .map(|range| js_number(&key[range]));
        if let Some(index) = index
            && index < 16.0
        {
            scheme.set_palette(index as usize, parse_color(Some(value)));
            continue;
        }
        if let Some(field) = kitty_special_field(key) {
            set_special(&mut scheme, field, parse_color(Some(value)));
        }
    }
    scheme
}

fn read_alacritty(text: &str) -> PartialScheme {
    static TOML_HEADER: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*\[colors\.(\w+)\]\s*$", "").ok());
    static YAML_HEADER: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*(\w+)\s*:\s*$", "").ok());
    static ENTRY: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*(\w+)\s*[=:]\s*(\S+)", "").ok());

    let mut scheme = PartialScheme::default();
    let mut section = String::new();
    for line in split_lines(text) {
        let header = TOML_HEADER
            .as_ref()
            .and_then(|pattern| pattern.find(line))
            .and_then(|captures| captures.group(1))
            .or_else(|| {
                YAML_HEADER
                    .as_ref()
                    .and_then(|pattern| pattern.find(line))
                    .and_then(|captures| captures.group(1))
            });
        if let Some(range) = header {
            section = line[range].to_string();
            continue;
        }
        let Some(captures) = ENTRY.as_ref().and_then(|pattern| pattern.find(line)) else {
            continue;
        };
        let key = captures
            .group(1)
            .map(|range| &line[range])
            .unwrap_or_default();
        let raw = captures
            .group(2)
            .map(|range| &line[range])
            .unwrap_or_default();
        let value = parse_color(Some(raw));
        let index = ANSI_NAMES.iter().position(|name| *name == key);
        if section == "primary" && (key == "background" || key == "foreground") {
            if key == "background" {
                scheme.background = value;
            } else {
                scheme.foreground = value;
            }
        } else if (section == "normal" || section == "bright") && index.is_some() {
            let index = index.unwrap_or(0) + if section == "bright" { 8 } else { 0 };
            scheme.set_palette(index, value);
        } else if section == "cursor" {
            if key == "cursor" {
                scheme.cursor = value;
            } else if key == "text" {
                scheme.cursor_text = value;
            }
        } else if section == "selection" {
            if key == "background" {
                scheme.selection_background = value;
            } else if key == "text" {
                scheme.selection_foreground = value;
            }
        }
    }
    scheme
}

fn read_iterm(text: &str) -> PartialScheme {
    static ENTRY: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::with_flags(r"<key>([^<]+)</key>\s*<dict>([\s\S]*?)</dict>", "g").ok()
    });
    static RED: LazyLock<Option<Regex>> = LazyLock::new(|| component_pattern("Red"));
    static GREEN: LazyLock<Option<Regex>> = LazyLock::new(|| component_pattern("Green"));
    static BLUE: LazyLock<Option<Regex>> = LazyLock::new(|| component_pattern("Blue"));

    let mut colors = HashMap::new();
    if let Some(pattern) = ENTRY.as_ref() {
        for captures in pattern.find_iter(text) {
            let Some(key) = captures.group(1) else {
                continue;
            };
            let Some(body) = captures.group(2) else {
                continue;
            };
            let body = &text[body];
            let component = |pattern: &Option<Regex>| {
                pattern
                    .as_ref()
                    .and_then(|pattern| pattern.find(body))
                    .and_then(|captures| captures.group(1))
                    .map(|range| js_number(&body[range]))
                    .unwrap_or(f64::NAN)
            };
            let red = component(&RED);
            let green = component(&GREEN);
            let blue = component(&BLUE);
            if red.is_nan() || green.is_nan() || blue.is_nan() {
                continue;
            }
            let channel = |value: f64| {
                let value = js_round(value.clamp(0.0, 1.0) * 255.0) as u8;
                format!("{value:02x}")
            };
            colors.insert(
                js_trim(&text[key]).to_string(),
                format!("#{}{}{}", channel(red), channel(green), channel(blue)),
            );
        }
    }

    let color = |key: &str| -> Option<String> {
        colors
            .get(key)
            .or_else(|| {
                let dark = format!("{key} (Dark)");
                colors.get(&dark)
            })
            .cloned()
    };
    let mut scheme = PartialScheme::default();
    for index in 0..16 {
        scheme.set_palette(index, color(&format!("Ansi {index} Color")));
    }
    scheme.background = color("Background Color");
    scheme.foreground = color("Foreground Color");
    scheme.cursor = color("Cursor Color");
    scheme.cursor_text = color("Cursor Text Color");
    scheme.selection_background = color("Selection Color");
    scheme.selection_foreground = color("Selected Text Color");
    scheme
}

fn component_pattern(name: &str) -> Option<Regex> {
    Regex::with_flags(
        &format!("<key>{name} Component</key>\\s*<(?:real|integer)>([^<]+)</"),
        "",
    )
    .ok()
}

fn parse_jsonc(text: &str) -> Result<Value, ()> {
    static STRING: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r#"^"(?:\\.|[^"\\])*""#, "").ok());
    static TRAILING_COMMA: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r",(\s*[}\]])", "g").ok());

    let mut result = String::new();
    let mut index = 0;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let rest = &text[index..];
            if let Some(captures) = STRING.as_ref().and_then(|pattern| pattern.find(rest)) {
                let range = captures.range();
                result.push_str(&rest[range.clone()]);
                index += range.end;
            } else {
                let character = rest.chars().next().unwrap_or('"');
                result.push(character);
                index += character.len_utf8();
            }
        } else if text[index..].starts_with("//") {
            index = match text[index..].find('\n') {
                Some(offset) => index + offset,
                None => bytes.len(),
            };
        } else if text[index..].starts_with("/*") {
            index = match text[index..].find("*/") {
                Some(offset) => index + offset + 2,
                None => bytes.len(),
            };
        } else {
            let character = text[index..].chars().next().unwrap_or('\0');
            result.push(character);
            index += character.len_utf8();
        }
    }
    let result = TRAILING_COMMA
        .as_ref()
        .map(|pattern| pattern.replace_all(&result, "$1"))
        .unwrap_or(result);
    serde_json::from_str(&result).map_err(|_| ())
}

fn read_windows_terminal(json: &Value) -> Vec<PartialScheme> {
    let found: Vec<&Value> = if let Some(array) = json.as_array() {
        array.iter().collect()
    } else if json.get("schemes").and_then(Value::as_array).is_some() {
        json.get("schemes")
            .and_then(Value::as_array)
            .map(|array| array.iter().collect())
            .unwrap_or_default()
    } else {
        vec![json]
    };

    found
        .into_iter()
        .filter(|entry| !entry.is_null() && (entry.is_object() || entry.is_array()))
        .map(|entry| {
            if entry.is_array() {
                return PartialScheme::default();
            }
            let value = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .and_then(|value| parse_color(Some(value)))
            };
            let mut scheme = PartialScheme {
                name: entry
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                ..PartialScheme::default()
            };
            for (index, name) in ANSI_NAMES.iter().enumerate() {
                let alias: &str = if *name == "magenta" { "purple" } else { name };
                let first = alias.chars().next().unwrap_or('x');
                let bright = format!("bright{}{}", first.to_ascii_uppercase(), &alias[1..]);
                let bright_name = format!(
                    "bright{}{}",
                    name.chars().next().unwrap_or('x').to_ascii_uppercase(),
                    &name[1..]
                );
                scheme.set_palette(index, value(alias).or_else(|| value(name)));
                scheme.set_palette(index + 8, value(&bright).or_else(|| value(&bright_name)));
            }
            scheme.background = value("background");
            scheme.foreground = value("foreground");
            scheme.cursor = value("cursorColor");
            scheme.selection_background = value("selectionBackground");
            scheme
        })
        .collect()
}

fn detect_format(text: &str) -> Option<ThemeFileFormat> {
    static TOML_SECTION: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*\[colors\.\w+\]", "m").ok());
    static YAML_SECTION: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*(primary|normal|bright)\s*:\s*$", "m").ok());
    static PLIST: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"<plist[\s>]", "").ok());
    static ITERM_KEY: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"<key>Ansi 0 Color", "").ok());
    static GHOSTTY: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*palette\s*=\s*\d+\s*=", "m").ok());
    static KITTY: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::with_flags(r"^\s*color\d+\s+\S+", "m").ok());

    let trimmed = js_trim(text);
    let matches = |pattern: &Option<Regex>| {
        pattern
            .as_ref()
            .is_some_and(|pattern| pattern.find(trimmed).is_some())
    };
    if matches(&TOML_SECTION) || matches(&YAML_SECTION) {
        return Some(ThemeFileFormat::Alacritty);
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Some(ThemeFileFormat::WindowsTerminal);
    }
    if matches(&PLIST) || matches(&ITERM_KEY) {
        return Some(ThemeFileFormat::Iterm);
    }
    if matches(&GHOSTTY) {
        return Some(ThemeFileFormat::Ghostty);
    }
    if matches(&KITTY) {
        return Some(ThemeFileFormat::Kitty);
    }
    None
}

enum SpecialField {
    Background,
    Foreground,
    Cursor,
    CursorText,
    SelectionBackground,
    SelectionForeground,
}

fn ghostty_special_field(key: &str) -> Option<SpecialField> {
    match key {
        "background" => Some(SpecialField::Background),
        "foreground" => Some(SpecialField::Foreground),
        "cursor-color" => Some(SpecialField::Cursor),
        "cursor-text" => Some(SpecialField::CursorText),
        "selection-background" => Some(SpecialField::SelectionBackground),
        "selection-foreground" => Some(SpecialField::SelectionForeground),
        _ => None,
    }
}

fn kitty_special_field(key: &str) -> Option<SpecialField> {
    match key {
        "background" => Some(SpecialField::Background),
        "foreground" => Some(SpecialField::Foreground),
        "cursor" => Some(SpecialField::Cursor),
        "cursor_text_color" => Some(SpecialField::CursorText),
        "selection_background" => Some(SpecialField::SelectionBackground),
        "selection_foreground" => Some(SpecialField::SelectionForeground),
        _ => None,
    }
}

fn set_special(scheme: &mut PartialScheme, field: SpecialField, value: Option<String>) {
    match field {
        SpecialField::Background => scheme.background = value,
        SpecialField::Foreground => scheme.foreground = value,
        SpecialField::Cursor => scheme.cursor = value,
        SpecialField::CursorText => scheme.cursor_text = value,
        SpecialField::SelectionBackground => scheme.selection_background = value,
        SpecialField::SelectionForeground => scheme.selection_foreground = value,
    }
}

#[cfg(test)]
mod tests;
