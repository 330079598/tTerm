use serde::{Deserialize, Serialize};

/// How the panes of a split tab look.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct PaneConfig {
    /// Each pane of a split tab shows a header naming it.
    pub show_headers: bool,
    /// How much the panes without the keyboard fade, in percent; 0 leaves
    /// them be.
    pub dim_inactive_percent: u8,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            show_headers: true,
            dim_inactive_percent: 0,
        }
    }
}
