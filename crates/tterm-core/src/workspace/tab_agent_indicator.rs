//! The animations a tab shows while an AI agent works or waits, from
//! `src/lib/tabAgentIndicator.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An animation a tab can show while its agent works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabAgentWorkingStyle {
    /// A rotating ring.
    Ring,
    /// A spinning disc.
    Disc,
    /// A comet circling the tab.
    Comet,
    /// A dot orbiting the tab.
    Orbit,
    /// A pulsing mark.
    Pulse,
    /// Three blinking dots.
    Dots,
    /// A braille spinner.
    Braille,
    /// A walking crab.
    Crab,
}

/// Every working animation, in the order the settings list them.
pub const TAB_AGENT_WORKING_STYLES: &[TabAgentWorkingStyle] = &[
    TabAgentWorkingStyle::Ring,
    TabAgentWorkingStyle::Disc,
    TabAgentWorkingStyle::Comet,
    TabAgentWorkingStyle::Orbit,
    TabAgentWorkingStyle::Pulse,
    TabAgentWorkingStyle::Dots,
    TabAgentWorkingStyle::Braille,
    TabAgentWorkingStyle::Crab,
];

/// A mark a tab can show while its agent waits on the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabAgentWaitingStyle {
    /// Three bars.
    Bars,
    /// A ring.
    Ring,
    /// A still mark.
    Still,
    /// A waiting crab.
    Crab,
}

impl TabAgentWorkingStyle {
    fn from_id(value: &str) -> Option<Self> {
        match value {
            "ring" => Some(Self::Ring),
            "disc" => Some(Self::Disc),
            "comet" => Some(Self::Comet),
            "orbit" => Some(Self::Orbit),
            "pulse" => Some(Self::Pulse),
            "dots" => Some(Self::Dots),
            "braille" => Some(Self::Braille),
            "crab" => Some(Self::Crab),
            _ => None,
        }
    }
}

impl TabAgentWaitingStyle {
    fn from_id(value: &str) -> Option<Self> {
        match value {
            "bars" => Some(Self::Bars),
            "ring" => Some(Self::Ring),
            "still" => Some(Self::Still),
            "crab" => Some(Self::Crab),
            _ => None,
        }
    }
}

/// Every waiting mark, in the order the settings list them.
pub const TAB_AGENT_WAITING_STYLES: &[TabAgentWaitingStyle] = &[
    TabAgentWaitingStyle::Bars,
    TabAgentWaitingStyle::Ring,
    TabAgentWaitingStyle::Still,
    TabAgentWaitingStyle::Crab,
];

/// How a tab shows an AI agent at work or waiting, part of the app config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabAgentIndicatorConfig {
    /// The animation shown while the agent works.
    pub working: TabAgentWorkingStyle,
    /// The mark shown while the agent waits on the user.
    pub waiting: TabAgentWaitingStyle,
    /// A `#rrggbb` color, or empty for the theme's primary color.
    pub working_color: String,
    /// A `#rrggbb` color, or empty for the theme's warning color.
    pub waiting_color: String,
}

/// The indicator settings used when a saved value is missing.
pub const DEFAULT_TAB_AGENT_INDICATOR: TabAgentIndicatorConfig = TabAgentIndicatorConfig {
    working: TabAgentWorkingStyle::Ring,
    waiting: TabAgentWaitingStyle::Bars,
    working_color: String::new(),
    waiting_color: String::new(),
};

fn normalize_color(value: Option<&Value>) -> String {
    let Some(value) = value.and_then(Value::as_str) else {
        return String::new();
    };
    let bytes = value.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        value.to_ascii_lowercase()
    } else {
        String::new()
    }
}

/// A saved indicator config with unknown styles and malformed colors set back
/// to the defaults.
pub fn normalize_tab_agent_indicator(value: &Value) -> TabAgentIndicatorConfig {
    let saved = value.as_object();
    let working = saved
        .and_then(|saved| saved.get("working"))
        .and_then(Value::as_str)
        .and_then(TabAgentWorkingStyle::from_id)
        .unwrap_or(DEFAULT_TAB_AGENT_INDICATOR.working);
    let waiting = saved
        .and_then(|saved| saved.get("waiting"))
        .and_then(Value::as_str)
        .and_then(TabAgentWaitingStyle::from_id)
        .unwrap_or(DEFAULT_TAB_AGENT_INDICATOR.waiting);
    TabAgentIndicatorConfig {
        working,
        waiting,
        working_color: normalize_color(saved.and_then(|saved| saved.get("working_color"))),
        waiting_color: normalize_color(saved.and_then(|saved| saved.get("waiting_color"))),
    }
}
