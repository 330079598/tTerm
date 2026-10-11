use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How local terminal tabs show their shell's icon. The frontend checks the
/// icon ids and colors it reads back; this only keeps what it saved.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TabIconConfig {
    /// Show an icon in front of each local tab's title.
    pub enabled: bool,
    /// The icon and color chosen for a shell, by its lowercased name (`pwsh`,
    /// `git bash`). A shell missing here keeps its built-in look.
    pub shells: BTreeMap<String, TabIconStyle>,
}

impl Default for TabIconConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shells: BTreeMap::new(),
        }
    }
}

/// An icon and color chosen for a shell. A missing one keeps the built-in
/// choice; an empty color means no tint.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct TabIconStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}
