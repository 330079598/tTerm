//! How tabs and split panes look, from `src/lib/tabSettings.ts`.

use serde::Serialize;
use serde_json::Value;

use tterm_terminal::theme::js::js_round;

use super::pane_templates::{PaneLayoutTemplate, normalize_pane_templates};
use super::tab_agent_indicator::{
    DEFAULT_TAB_AGENT_INDICATOR, TabAgentIndicatorConfig, normalize_tab_agent_indicator,
};
use super::tab_icons::{TabIconConfig, normalize_tab_icon_config};

/// The standard tab width used when a saved value is missing.
const DEFAULT_TAB_STANDARD_WIDTH: u16 = 120;

/// How much the panes without the keyboard fade, in percent; 0 leaves them be.
pub const INACTIVE_PANE_DIM_OPTIONS: [u8; 4] = [0, 15, 30, 45];

/// How the panes of a split tab look, and the layouts saved to split tabs with.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaneSettings {
    /// Each pane of a split tab shows a header naming it, with buttons to
    /// split, zoom and close it.
    pub show_headers: bool,
    /// How much the panes without the keyboard fade, in percent.
    pub dim_inactive_percent: u8,
    /// The split layouts saved to split tabs with.
    pub templates: Vec<PaneLayoutTemplate>,
}

/// The settings for how tabs look, part of the app config.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TabSettings {
    /// How wide tabs are: `adaptive` for each as wide as its title, or
    /// `standard` for all the same width.
    pub tab_width_mode: String,
    /// The width in pixels standard tabs use.
    pub tab_standard_width: u16,
    /// Show a tab's full title and connection in a tooltip on hover.
    pub tab_hover_tooltip: bool,
    /// How local terminal tabs show their shell's icon.
    pub tab_icons: TabIconConfig,
    /// How a tab shows an AI agent at work or waiting for the user.
    pub tab_agent_indicator: TabAgentIndicatorConfig,
    /// How split panes look, and the saved split layouts.
    pub panes: PaneSettings,
}

/// The tab settings before the user changes anything.
pub fn default_tab_settings() -> TabSettings {
    TabSettings {
        tab_width_mode: "adaptive".to_string(),
        tab_standard_width: DEFAULT_TAB_STANDARD_WIDTH,
        tab_hover_tooltip: false,
        tab_icons: TabIconConfig::default(),
        tab_agent_indicator: DEFAULT_TAB_AGENT_INDICATOR,
        panes: PaneSettings {
            show_headers: true,
            dim_inactive_percent: 0,
            templates: Vec::new(),
        },
    }
}

/// The tab settings from a saved config, each one valid.
pub fn normalize_tab_settings(value: &Value) -> TabSettings {
    let saved = value.as_object();
    TabSettings {
        tab_width_mode: if saved
            .and_then(|saved| saved.get("tab_width_mode"))
            .and_then(Value::as_str)
            == Some("standard")
        {
            "standard".to_string()
        } else {
            "adaptive".to_string()
        },
        tab_standard_width: normalize_tab_standard_width(
            saved.and_then(|saved| saved.get("tab_standard_width")),
        ),
        tab_hover_tooltip: saved
            .and_then(|saved| saved.get("tab_hover_tooltip"))
            .and_then(Value::as_bool)
            == Some(true),
        tab_icons: normalize_tab_icon_config(saved_value(saved, "tab_icons")),
        tab_agent_indicator: normalize_tab_agent_indicator(saved_value(
            saved,
            "tab_agent_indicator",
        )),
        panes: normalize_pane_settings(saved_value(saved, "panes")),
    }
}

fn saved_value<'a>(saved: Option<&'a serde_json::Map<String, Value>>, key: &str) -> &'a Value {
    saved
        .and_then(|saved| saved.get(key))
        .unwrap_or(&Value::Null)
}

/// A standard tab width in pixels, rounded into 80-300; 120 when unset.
fn normalize_tab_standard_width(value: Option<&Value>) -> u16 {
    let Some(value) = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
    else {
        return DEFAULT_TAB_STANDARD_WIDTH;
    };

    js_round(value).clamp(80.0, 300.0) as u16
}

fn normalize_pane_settings(value: &Value) -> PaneSettings {
    let panes = value.as_object();
    let dim = panes
        .and_then(|panes| panes.get("dim_inactive_percent"))
        .and_then(Value::as_f64)
        .filter(|dim| dim.is_finite());
    PaneSettings {
        show_headers: panes
            .and_then(|panes| panes.get("show_headers"))
            .and_then(Value::as_bool)
            != Some(false),
        dim_inactive_percent: dim
            .map(|dim| js_round(dim).clamp(0.0, 60.0) as u8)
            .unwrap_or(0),
        templates: normalize_pane_templates(saved_value(panes, "templates")),
    }
}

#[cfg(test)]
mod tests;
