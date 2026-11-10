//! Keyword highlight configuration and immutable editing helpers.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

/// Palette names accepted by keyword highlight rules.
pub const HIGHLIGHT_PALETTE_COLORS: [&str; 16] = [
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
pub const HIGHLIGHT_SET_FOLLOW_DEFAULT: &str = "default";
pub const HIGHLIGHT_SET_OFF: &str = "off";
pub const BUILTIN_HIGHLIGHT_SET_ID: &str = "builtin";
pub const MAX_HIGHLIGHT_SETS: usize = 50;
pub const MAX_HIGHLIGHT_RULES: usize = 100;
pub const MAX_HIGHLIGHT_PATTERN_LENGTH: usize = 1000;

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
/// Persisted keyword highlight settings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct KeywordHighlightConfig {
    pub enabled: bool,
    pub default_set: String,
    pub alternate_screen: bool,
    pub sets: Option<Vec<HighlightSet>>,
}
impl Default for KeywordHighlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_set: BUILTIN_HIGHLIGHT_SET_ID.into(),
            alternate_screen: false,
            sets: None,
        }
    }
}

const IPV4_PATTERN: &str = r"(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}(?:/(?:3[0-2]|[12]?\d))?";
const IPV6_PATTERN: &str = r"(?:(?:[\da-f]{1,4}:){7}[\da-f]{1,4}|(?:[\da-f]{1,4}:){1,7}:|(?:[\da-f]{1,4}:){1,6}:[\da-f]{1,4}|(?:[\da-f]{1,4}:){1,5}(?::[\da-f]{1,4}){1,2}|(?:[\da-f]{1,4}:){1,4}(?::[\da-f]{1,4}){1,3}|(?:[\da-f]{1,4}:){1,3}(?::[\da-f]{1,4}){1,4}|(?:[\da-f]{1,4}:){1,2}(?::[\da-f]{1,4}){1,5}|[\da-f]{1,4}:(?::[\da-f]{1,4}){1,6}|:(?::[\da-f]{1,4}){1,7})(?:/\d{1,3})?";
const MAC_PATTERN: &str = r"(?:[\da-f]{2}(?::[\da-f]{2}){5}|[\da-f]{2}(?:-[\da-f]{2}){5}|[\da-f]{4}\.[\da-f]{4}\.[\da-f]{4})";
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
        id: BUILTIN_HIGHLIGHT_SET_ID.into(),
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
/// Returns the display name of a set, translating the built-in and unnamed names.
pub fn highlight_set_name(set: &HighlightSet, translate: impl Fn(&str) -> String) -> String {
    if !set.name.is_empty() {
        set.name.clone()
    } else if set.id == BUILTIN_HIGHLIGHT_SET_ID {
        translate("keywordHighlight.builtinSetName")
    } else {
        translate("keywordHighlight.unnamedSet")
    }
}
/// Returns the default configuration.
pub fn default_keyword_highlight_config() -> KeywordHighlightConfig {
    KeywordHighlightConfig::default()
}
/// Returns configured sets, or the built-in set before the first edit.
pub fn highlight_sets(config: &KeywordHighlightConfig) -> Vec<HighlightSet> {
    config
        .sets
        .clone()
        .unwrap_or_else(|| vec![builtin_highlight_set()])
}
/// Resolves a connection choice to a highlight set.
pub fn resolve_highlight_set(
    config: &KeywordHighlightConfig,
    choice: Option<&str>,
) -> Option<HighlightSet> {
    if !config.enabled || choice == Some(HIGHLIGHT_SET_OFF) {
        return None;
    }
    let sets = highlight_sets(config);
    let wanted = choice
        .filter(|v| *v != HIGHLIGHT_SET_FOLLOW_DEFAULT)
        .unwrap_or(&config.default_set);
    sets.iter()
        .find(|s| s.id == wanted)
        .cloned()
        .or_else(|| sets.iter().find(|s| s.id == config.default_set).cloned())
        .or_else(|| sets.first().cloned())
}
fn normalize_color(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    if HIGHLIGHT_PALETTE_COLORS.contains(&value) {
        return Some(value.into());
    }
    if value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Some(value.to_ascii_lowercase());
    }
    None
}
fn normalize_rule(value: &Value) -> Option<HighlightRule> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?.to_string();
    if id.is_empty() || !object.get("pattern")?.is_string() {
        return None;
    }
    let pattern = object
        .get("pattern")?
        .as_str()?
        .chars()
        .take(MAX_HIGHLIGHT_PATTERN_LENGTH)
        .collect();
    Some(HighlightRule {
        id,
        enabled: object.get("enabled").and_then(Value::as_bool) != Some(false),
        pattern,
        kind: if object.get("kind").and_then(Value::as_str) == Some("regex") {
            "regex".into()
        } else {
            "keyword".into()
        },
        case_sensitive: object.get("case_sensitive").and_then(Value::as_bool) == Some(true),
        whole_word: object.get("whole_word").and_then(Value::as_bool) == Some(true),
        foreground: normalize_color(object.get("foreground")),
        background: normalize_color(object.get("background")),
    })
}
fn normalize_set(value: &Value) -> Option<HighlightSet> {
    let object = value.as_object()?;
    let id = object.get("id")?.as_str()?.to_string();
    if id.is_empty() {
        return None;
    }
    let rules = object
        .get("rules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_rule)
                .take(MAX_HIGHLIGHT_RULES)
                .collect()
        })
        .unwrap_or_default();
    Some(HighlightSet {
        id,
        name: object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        rules,
    })
}
/// Reads possibly malformed persisted data using the frontend's permissive defaults.
pub fn normalize_keyword_highlight_config(value: &Value) -> KeywordHighlightConfig {
    let defaults = KeywordHighlightConfig::default();
    let Some(object) = value.as_object() else {
        return defaults;
    };
    let sets = object.get("sets").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(normalize_set)
            .take(MAX_HIGHLIGHT_SETS)
            .collect()
    });
    KeywordHighlightConfig {
        enabled: object.get("enabled").and_then(Value::as_bool) != Some(false),
        default_set: object
            .get("default_set")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or(&defaults.default_set)
            .into(),
        alternate_screen: object.get("alternate_screen").and_then(Value::as_bool) == Some(true),
        sets,
    }
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// Creates a unique id for a newly inserted rule or set.
pub fn new_highlight_id() -> String {
    format!("rust-highlight-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}
fn with_sets(config: &KeywordHighlightConfig, sets: Vec<HighlightSet>) -> KeywordHighlightConfig {
    KeywordHighlightConfig {
        sets: Some(sets),
        ..config.clone()
    }
}
/// Changes one set, preserving the config when the id is absent.
pub fn update_highlight_set(
    config: &KeywordHighlightConfig,
    set_id: &str,
    change: impl Fn(&HighlightSet) -> HighlightSet,
) -> KeywordHighlightConfig {
    let sets = highlight_sets(config);
    if !sets.iter().any(|set| set.id == set_id) {
        return config.clone();
    }
    with_sets(
        config,
        sets.into_iter()
            .map(|set| if set.id == set_id { change(&set) } else { set })
            .collect(),
    )
}
/// Adds a set, optionally copying rules with fresh ids.
pub fn add_highlight_set(
    config: &KeywordHighlightConfig,
    name: String,
    source: Option<&HighlightSet>,
) -> Option<(KeywordHighlightConfig, String)> {
    let mut sets = highlight_sets(config);
    if sets.len() >= MAX_HIGHLIGHT_SETS {
        return None;
    }
    let id = new_highlight_id();
    let rules = source
        .map(|s| {
            s.rules
                .iter()
                .map(|rule| HighlightRule {
                    id: new_highlight_id(),
                    ..rule.clone()
                })
                .collect()
        })
        .unwrap_or_default();
    sets.push(HighlightSet {
        id: id.clone(),
        name,
        rules,
    });
    Some((with_sets(config, sets), id))
}
/// Deletes a set unless it would leave no sets.
pub fn delete_highlight_set(
    config: &KeywordHighlightConfig,
    set_id: &str,
) -> KeywordHighlightConfig {
    let sets = highlight_sets(config);
    let remaining: Vec<_> = sets
        .iter()
        .filter(|set| set.id != set_id)
        .cloned()
        .collect();
    if remaining.is_empty() || remaining.len() == sets.len() {
        return config.clone();
    }
    let default_set = if remaining.iter().any(|set| set.id == config.default_set) {
        config.default_set.clone()
    } else {
        remaining[0].id.clone()
    };
    KeywordHighlightConfig {
        default_set,
        sets: Some(remaining),
        ..config.clone()
    }
}
/// Restores only the built-in set's rules.
pub fn restore_builtin_rules(set: &HighlightSet) -> HighlightSet {
    if set.id == BUILTIN_HIGHLIGHT_SET_ID {
        HighlightSet {
            rules: builtin_highlight_set().rules,
            ..set.clone()
        }
    } else {
        set.clone()
    }
}
/// Creates the default editable rule.
pub fn new_highlight_rule() -> HighlightRule {
    HighlightRule {
        id: new_highlight_id(),
        enabled: true,
        pattern: String::new(),
        kind: "keyword".into(),
        case_sensitive: false,
        whole_word: true,
        foreground: Some("red".into()),
        background: None,
    }
}
/// Adds a rule until the configured limit.
pub fn add_highlight_rule(set: &HighlightSet, rule: HighlightRule) -> HighlightSet {
    if set.rules.len() >= MAX_HIGHLIGHT_RULES {
        return set.clone();
    }
    HighlightSet {
        rules: set.rules.iter().cloned().chain([rule]).collect(),
        ..set.clone()
    }
}
/// Fields accepted when updating a rule.
#[derive(Clone, Debug, Default)]
pub struct HighlightRuleChanges {
    pub enabled: Option<bool>,
    pub pattern: Option<String>,
    pub kind: Option<String>,
    pub case_sensitive: Option<bool>,
    pub whole_word: Option<bool>,
    pub foreground: Option<Option<String>>,
    pub background: Option<Option<String>>,
}
/// Updates one rule.
pub fn update_highlight_rule(
    set: &HighlightSet,
    rule_id: &str,
    changes: HighlightRuleChanges,
) -> HighlightSet {
    let HighlightRuleChanges {
        enabled,
        pattern,
        kind,
        case_sensitive,
        whole_word,
        foreground,
        background,
    } = changes;
    let rules = set
        .rules
        .iter()
        .map(|rule| {
            if rule.id != rule_id {
                return rule.clone();
            }
            let mut updated = rule.clone();
            if let Some(value) = enabled {
                updated.enabled = value;
            }
            if let Some(value) = &pattern {
                updated.pattern = value.clone();
            }
            if let Some(value) = &kind {
                updated.kind = value.clone();
            }
            if let Some(value) = case_sensitive {
                updated.case_sensitive = value;
            }
            if let Some(value) = whole_word {
                updated.whole_word = value;
            }
            if let Some(value) = &foreground {
                updated.foreground = value.clone();
            }
            if let Some(value) = &background {
                updated.background = value.clone();
            }
            updated
        })
        .collect();
    HighlightSet {
        rules,
        ..set.clone()
    }
}
/// Removes a rule.
pub fn remove_highlight_rule(set: &HighlightSet, rule_id: &str) -> HighlightSet {
    HighlightSet {
        rules: set
            .rules
            .iter()
            .filter(|r| r.id != rule_id)
            .cloned()
            .collect(),
        ..set.clone()
    }
}
/// Moves a rule by one position, preserving the set when out of bounds.
pub fn move_highlight_rule(set: &HighlightSet, rule_id: &str, offset: i8) -> HighlightSet {
    let Some(index) = set.rules.iter().position(|r| r.id == rule_id) else {
        return set.clone();
    };
    let target = index as i8 + offset;
    if target < 0 || target as usize >= set.rules.len() {
        return set.clone();
    }
    let mut rules = set.rules.clone();
    rules.swap(index, target as usize);
    HighlightSet {
        rules,
        ..set.clone()
    }
}

#[cfg(test)]
mod tests;
