//! Tab and context-menu value types, from `src/types/tab.ts`.
//!
//! Only the fields the tab navigation, closing and menu modules read are
//! installed. The second P1-11d group completes the tab model here.

use serde::{Deserialize, Serialize};

use crate::keymap::actions::KeymapActionId;

/// A tab that runs a terminal, an SSH connection or a serial connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    /// A shell running on this machine.
    Terminal,
    /// A session to a remote host over SSH.
    Ssh,
    /// A session over a serial port.
    Serial,
}

/// Every kind of tab the bar can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabType {
    /// A shell running on this machine.
    Terminal,
    /// A session to a remote host over SSH.
    Ssh,
    /// A session over a serial port.
    Serial,
    /// The settings page.
    Settings,
    /// The tunnels page.
    Tunnels,
    /// The terminal logs page.
    Logs,
    /// The editor for a remote file.
    RemoteFileEditor,
}

/// The connection fields the tab menu reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TabConnection {
    /// The saved connection the tab was opened from.
    pub profile_id: Option<String>,
}

/// A tab of the tab bar, with the fields this card's menu model reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    /// The tab's unique id, which pane trees refer to.
    pub id: String,
    /// The title shown in the tab bar.
    pub title: String,
    /// What the tab shows.
    #[serde(rename = "type")]
    pub tab_type: TabType,
    /// Whether the tab is in front in its group.
    pub is_active: bool,
    /// Whether the connection header stays visible; missing means pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_header_pinned: Option<bool>,
    /// The saved connection the tab was opened from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<TabConnection>,
}

impl Tab {
    /// A tab with the fields the menu model needs.
    pub fn new(id: impl Into<String>, title: impl Into<String>, tab_type: TabType) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tab_type,
            is_active: false,
            connection_header_pinned: None,
            connection: None,
        }
    }
}

/// An item of a tab or pane context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabContextMenuAction {
    /// The translated label.
    pub label: String,
    /// The action identifier the menu handler receives.
    pub action: String,
    /// The icon name, when the item shows one.
    pub icon: Option<String>,
    /// Whether the item is a separator rather than an action.
    pub separator: bool,
    /// Whether the item is disabled; missing means enabled.
    pub disabled: Option<bool>,
    /// The keyboard shortcut shown beside the item.
    pub shortcut: Option<KeymapActionId>,
}

/// Whether a tab is one of the singleton app pages.
pub fn is_page_tab(tab: &Tab) -> bool {
    matches!(
        tab.tab_type,
        TabType::Settings | TabType::Tunnels | TabType::Logs
    )
}

/// Whether a tab runs a local, SSH or serial terminal session.
pub fn is_session_tab(tab: &Tab) -> bool {
    matches!(
        tab.tab_type,
        TabType::Terminal | TabType::Ssh | TabType::Serial
    )
}
