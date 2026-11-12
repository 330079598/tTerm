//! Specification tests transcribed from `src/lib/*.test.ts`.

mod app_navigation;
mod pane_layout;
mod pane_navigation;
mod pane_templates;
mod persistence;
mod tab_agent_indicator;
mod tab_attention;
mod tab_closing;
mod tab_menu_actions;
mod tab_navigation;
mod tab_nudge;

use crate::workspace::tab::{Tab, TabType};

pub(crate) fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(crate) fn tab(id: &str, title: &str, tab_type: TabType) -> Tab {
    Tab::new(id, title, tab_type)
}
