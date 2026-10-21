use serde::{Deserialize, Serialize};

/// The built-in highlight set's id; the frontend holds its rules.
const BUILTIN_SET_ID: &str = "builtin";

/// Keyword highlighting of terminal output, in named sets of rules. The
/// frontend checks the patterns and colors it reads back; this only keeps what
/// it saved.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct KeywordHighlightConfig {
    pub enabled: bool,
    /// The set terminals use unless their connection picks another.
    pub default_set: String,
    /// Also highlight full-screen programs such as vim, htop or tmux.
    pub alternate_screen: bool,
    /// None until the user edits a set, while the built-in set applies; the
    /// built-in rules then improve with tTerm instead of staying as saved.
    pub sets: Option<Vec<HighlightSet>>,
}

impl Default for KeywordHighlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_set: BUILTIN_SET_ID.to_string(),
            alternate_screen: false,
            sets: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct HighlightSet {
    pub id: String,
    /// Empty for the built-in set, which shows a translated name.
    pub name: String,
    pub rules: Vec<HighlightRule>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct HighlightRule {
    pub id: String,
    pub enabled: bool,
    pub pattern: String,
    /// "keyword" matches the pattern literally, "regex" as a JavaScript regex.
    pub kind: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// A terminal palette color name or `#rrggbb`; None keeps the text's color.
    pub foreground: Option<String>,
    pub background: Option<String>,
}

impl Default for HighlightRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            pattern: String::new(),
            kind: "keyword".to_string(),
            case_sensitive: false,
            whole_word: false,
            foreground: None,
            background: None,
        }
    }
}
