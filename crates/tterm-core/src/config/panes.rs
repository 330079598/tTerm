use serde::{Deserialize, Serialize};

/// How the panes of a split tab look, and the layouts saved to split tabs
/// with. The frontend checks the templates it reads back; this only keeps
/// what it saved.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct PaneConfig {
    /// Each pane of a split tab shows a header naming it.
    pub show_headers: bool,
    /// How much the panes without the keyboard fade, in percent; 0 leaves
    /// them be.
    pub dim_inactive_percent: u8,
    pub templates: Vec<PaneLayoutTemplate>,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            show_headers: true,
            dim_inactive_percent: 0,
            templates: Vec::new(),
        }
    }
}

/// A saved split layout: its name and the shape of its split tree, kept as
/// the frontend wrote it.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct PaneLayoutTemplate {
    pub id: String,
    pub name: String,
    pub root: serde_json::Value,
}
