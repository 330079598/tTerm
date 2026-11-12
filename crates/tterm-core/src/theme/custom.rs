//! Persisted custom themes: normalization, preset overrides and id checks.
//!
//! localStorage-backed helpers are not here; the native client reads the
//! legacy list through the P1-12 migration instead.

use serde_json::Value;

use tterm_terminal::theme::{
    OptionalText, PaletteText, TERMINAL_PALETTE_KEYS, THEME_COLOR_KEYS, ThemeColors,
};

use crate::theme::definitions::resolve_theme_definition;
use crate::theme::types::{
    CustomTheme, NormalizedThemeParts, PRESET_THEME_IDS, PresetTheme, RawObject,
};

/// Whether an id names one of the six built-in themes.
pub fn is_preset_theme_id(theme_id: &str) -> bool {
    PRESET_THEME_IDS.contains(&theme_id)
}

/// Replaces a preset's name, description and colors with a custom theme's,
/// keeping the preset id and `isCustom: false`.
pub fn merge_preset_theme_with_override(
    preset_theme: &PresetTheme,
    override_theme: Option<&CustomTheme>,
) -> PresetTheme {
    let Some(override_theme) = override_theme else {
        return preset_theme.clone();
    };
    PresetTheme {
        id: preset_theme.id.clone(),
        name: override_theme.name.clone(),
        description: override_theme.description.clone(),
        colors: override_theme.colors.clone(),
        terminal: override_theme.terminal.clone(),
        is_custom: false,
    }
}

/// Reads saved themes, dropping entries the frontend would drop and filling
/// in the defaults `normalizeCustomTheme` fills in.
pub fn normalize_custom_themes(themes: &[Value], now: i64) -> Vec<CustomTheme> {
    themes
        .iter()
        .filter_map(|theme| normalize_custom_theme(theme, now))
        .collect()
}

fn normalize_custom_theme(value: &Value, now: i64) -> Option<CustomTheme> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str().filter(|id| !id.is_empty())?;
    let name = object
        .get("name")?
        .as_str()
        .filter(|name| !name.is_empty())?;
    let raw_colors = spread_entries(object.get("colors")?)?;
    let raw_color_map: serde_json::Map<String, Value> = raw_colors.iter().cloned().collect();

    let base_theme = object.get("baseTheme");
    let base_id = base_theme
        .and_then(Value::as_str)
        .filter(|base| !base.is_empty())
        .unwrap_or("default");
    let fallback = resolve_theme_definition(base_id, &[]);

    let mut colors = fallback.colors().clone();
    let fallback_colors = serde_json::to_value(fallback.colors())
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut color_entries = Vec::with_capacity(THEME_COLOR_KEYS.len() + raw_color_map.len());
    for (index, key) in THEME_COLOR_KEYS.iter().enumerate() {
        let value = raw_color_map
            .get(*key)
            .or_else(|| fallback_colors.get(*key))
            .cloned()
            .unwrap_or(Value::Null);
        if let Some(text) = value.as_str() {
            set_color(&mut colors, index, text);
        }
        color_entries.push(((*key).to_string(), value));
    }
    for (key, value) in &raw_color_map {
        if !THEME_COLOR_KEYS.contains(&key.as_str()) {
            colors.extra.insert(key.clone(), value.clone());
            color_entries.push((key.clone(), value.clone()));
        }
    }

    let terminal_entries = object.get("terminal").and_then(spread_entries);
    let (terminal, raw_terminal) = match terminal_entries {
        Some(entries) => {
            let terminal = palette_text_from_entries(&entries);
            (terminal, RawObject::from_entries(entries))
        }
        None => {
            let terminal = fallback.terminal().clone();
            let map = serde_json::to_value(&terminal)
                .ok()
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            (terminal, RawObject::from_entries(map.into_iter().collect()))
        }
    };
    let raw_colors = RawObject::from_entries(color_entries);

    Some(CustomTheme::normalized(NormalizedThemeParts {
        id: id.to_string(),
        name: name.to_string(),
        description: optional_text(object.get("description")),
        colors,
        terminal,
        base_theme: optional_text(base_theme),
        created_at: normalized_timestamp(object.get("createdAt"), now),
        updated_at: normalized_timestamp(object.get("updatedAt"), now),
        raw_colors,
        raw_terminal,
    }))
}

fn optional_text(value: Option<&Value>) -> OptionalText {
    match value {
        None => OptionalText::Missing,
        Some(Value::Null) => OptionalText::Null,
        Some(Value::String(text)) => OptionalText::Value(text.clone()),
        Some(value) => OptionalText::Json(value.clone()),
    }
}

/// `value ?? Date.now()`: only a missing value or `null` asks for the clock.
fn normalized_timestamp(value: Option<&Value>, now: i64) -> Value {
    match value {
        None | Some(Value::Null) => Value::from(now),
        Some(Value::Number(number)) => canonical_number(number),
        Some(value) => value.clone(),
    }
}

/// JSON parsed from `1.0` carries a float, while JavaScript stores only the
/// number 1 and writes it back without a fraction.
fn canonical_number(number: &serde_json::Number) -> Value {
    let Some(value) = number.as_f64() else {
        return Value::Number(number.clone());
    };
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_992.0 {
        return Value::from(value as i64);
    }
    Value::Number(number.clone())
}

/// The entries a JavaScript object spread would copy.
///
/// JSON objects copy as they are; arrays become `"0"`, `"1"`, ... properties;
/// a string becomes one property per UTF-16 unit; other truthy primitives
/// copy nothing. A falsy value makes `theme.colors`/`theme.terminal` fail the
/// truthiness check in `normalizeCustomTheme`.
fn spread_entries(value: &Value) -> Option<Vec<(String, Value)>> {
    match value {
        Value::Null => None,
        Value::Bool(false) => None,
        Value::Number(number) if number.as_f64() == Some(0.0) => None,
        Value::String(text) if text.is_empty() => None,
        Value::Object(map) => Some(
            map.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        Value::Array(values) => Some(
            values
                .iter()
                .enumerate()
                .map(|(index, value)| (index.to_string(), value.clone()))
                .collect(),
        ),
        Value::String(text) => Some(
            text.encode_utf16()
                .enumerate()
                .map(|(index, unit)| {
                    let character = char::from_u32(u32::from(unit)).unwrap_or('\u{FFFD}');
                    (index.to_string(), Value::String(character.to_string()))
                })
                .collect(),
        ),
        Value::Bool(true) | Value::Number(_) => Some(Vec::new()),
    }
}

fn palette_text_from_entries(entries: &[(String, Value)]) -> PaletteText {
    let mut palette = PaletteText::default();
    for (key, value) in entries {
        if let Some(text) = value.as_str()
            && set_terminal_field(&mut palette, key, text)
        {
            continue;
        }
        if !TERMINAL_PALETTE_KEYS.contains(&key.as_str()) {
            palette.extra.insert(key.clone(), value.clone());
        }
    }
    palette
}

fn set_terminal_field(palette: &mut PaletteText, key: &str, value: &str) -> bool {
    let value = value.to_string();
    match key {
        "background" => palette.background = value,
        "foreground" => palette.foreground = value,
        "cursor" => palette.cursor = value,
        "cursorAccent" => palette.cursor_accent = OptionalText::Value(value),
        "selectionBackground" => palette.selection_background = value,
        "black" => palette.black = value,
        "red" => palette.red = value,
        "green" => palette.green = value,
        "yellow" => palette.yellow = value,
        "blue" => palette.blue = value,
        "magenta" => palette.magenta = value,
        "cyan" => palette.cyan = value,
        "white" => palette.white = value,
        "brightBlack" => palette.bright_black = value,
        "brightRed" => palette.bright_red = value,
        "brightGreen" => palette.bright_green = value,
        "brightYellow" => palette.bright_yellow = value,
        "brightBlue" => palette.bright_blue = value,
        "brightMagenta" => palette.bright_magenta = value,
        "brightCyan" => palette.bright_cyan = value,
        "brightWhite" => palette.bright_white = value,
        "selectionForeground" => palette.selection_foreground = OptionalText::Value(value),
        _ => return false,
    }
    true
}

fn set_color(colors: &mut ThemeColors, index: usize, value: &str) {
    let value = value.to_string();
    match index {
        0 => colors.background = value,
        1 => colors.foreground = value,
        2 => colors.card = value,
        3 => colors.card_foreground = value,
        4 => colors.popover = value,
        5 => colors.popover_foreground = value,
        6 => colors.primary = value,
        7 => colors.primary_foreground = value,
        8 => colors.secondary = value,
        9 => colors.secondary_foreground = value,
        10 => colors.muted = value,
        11 => colors.muted_foreground = value,
        12 => colors.accent = value,
        13 => colors.accent_foreground = value,
        14 => colors.destructive = value,
        15 => colors.destructive_foreground = value,
        16 => colors.success = value,
        17 => colors.success_foreground = value,
        18 => colors.warning = value,
        19 => colors.warning_foreground = value,
        20 => colors.border = value,
        21 => colors.input = value,
        22 => colors.ring = value,
        23 => colors.tab_background = value,
        24 => colors.tab_active = value,
        25 => colors.tab_active_border = value,
        26 => colors.tab_hover = value,
        27 => colors.titlebar = value,
        _ => {}
    }
}

#[cfg(test)]
mod tests;
