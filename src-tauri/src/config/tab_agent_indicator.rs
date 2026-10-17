use serde::{Deserialize, Serialize};

/// How a tab shows an AI agent at work in it or waiting for the user. The
/// frontend checks the styles and colors it reads back; this only keeps what
/// it saved.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TabAgentIndicatorConfig {
    /// The animation while the agent works: "disc", "ring", "comet", "orbit",
    /// "pulse", "dots" or "braille".
    pub working: String,
    /// The mark while the agent waits on the user: "bars", "ring" or "still".
    pub waiting: String,
    /// A `#rrggbb` color, or empty for the theme's primary color.
    pub working_color: String,
    /// A `#rrggbb` color, or empty for the theme's warning color.
    pub waiting_color: String,
}

impl Default for TabAgentIndicatorConfig {
    fn default() -> Self {
        Self {
            working: "disc".to_string(),
            waiting: "bars".to_string(),
            working_color: String::new(),
            waiting_color: String::new(),
        }
    }
}
