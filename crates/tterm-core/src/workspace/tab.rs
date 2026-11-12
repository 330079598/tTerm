//! Tab and context-menu value types, from `src/types/tab.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::distro::DistroId;
use crate::keymap::actions::KeymapActionId;
use crate::workspace::tab_icons::TabIconStyle;

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

/// The shell a local tab starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalShellType {
    /// Pick the platform's default shell.
    Auto,
    /// The Windows command shell.
    Cmd,
    /// Windows PowerShell.
    Powershell,
    /// PowerShell 7.
    Pwsh,
    /// A shell inside the Windows Subsystem for Linux.
    Wsl,
    /// Git Bash.
    GitBash,
    /// A shell named by its path.
    Custom,
}

/// The connection fields a tab carries, from `Tab["connection"]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TabConnection {
    /// The kind of connection.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<ConnectionType>,
    /// The saved connection the tab was opened from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    /// The saved connection's display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    /// The remote host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The remote SSH port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// The remote user name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// A password typed for this tab.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Whether a saved password is ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_saved_password: Option<bool>,
    /// Whether the password should be remembered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remember_password: Option<bool>,
    /// The SSH keepalive interval in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive_interval_secs: Option<u32>,
    /// The SSH keepalive retry count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keepalive_count_max: Option<u32>,
    /// Whether the server monitor is visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_monitor_visible: Option<bool>,
    /// The authentication method.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<String>,
    /// Whether agent forwarding is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_forward: Option<bool>,
    /// The private key path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    /// The private key passphrase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_passphrase: Option<String>,
    /// The OpenSSH certificate path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<String>,
    /// The shell a local tab starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell: Option<TerminalShellType>,
    /// A custom shell path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_custom_path: Option<String>,
    /// Arguments for a custom shell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_custom_args: Option<String>,
    /// The user's saved shell id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_id: Option<String>,
    /// The WSL distribution a WSL shell starts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_wsl_distro: Option<String>,
    /// The last directory a local shell reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// An AI session to resume after a restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_resume: Option<Value>,
    /// The ordered SSH jump host chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jump_hosts: Option<Value>,
    /// The remote shell's character encoding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// The terminal color theme.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_theme: Option<String>,
    /// Commands typed once the shell is ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_script: Option<String>,
    /// Whether shell integration is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_shell_integration: Option<bool>,
    /// The tab's session logging policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_log: Option<String>,
    /// Where the SFTP drawer opens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sftp_start_directory: Option<Value>,
    /// Whether SFTP works as root through sudo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sftp_sudo: Option<bool>,
    /// The keyword highlight set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight_set: Option<String>,
    /// The proxy used to reach the first host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<Value>,
    /// Serial port settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<Value>,
}

/// A tab of the tab bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub is_active: bool,
    /// Whether the tab's session has connected at least once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_connected: Option<bool>,
    /// Whether the tab's content has unsaved changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_modified: Option<bool>,
    /// The tab's icon name in the web client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The process id of a local shell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The generation of the session this tab opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_nonce: Option<u32>,
    /// Whether the connection header stays visible; missing means pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_header_pinned: Option<bool>,
    /// The base name a duplicated tab numbers its copies from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_base_title: Option<String>,
    /// Whether the title follows the shell's directory and command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_title: Option<bool>,
    /// The shell a local tab runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_name: Option<String>,
    /// The icon and color chosen for this tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_style: Option<TabIconStyle>,
    /// The reported Linux distribution, kept as a string to accept old data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro_id: Option<String>,
    /// Whether this is an unsaved quick connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_connect: Option<bool>,
    /// The authentication method a quick connection used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_connect_auth: Option<Value>,
    /// The settings section a page tab should show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_section: Option<String>,
    /// A counter that makes repeated settings requests observable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_request: Option<u32>,
    /// The saved connection the tab was opened from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<TabConnection>,
    /// The remote file a file-editor tab shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_file: Option<Value>,
}

impl Tab {
    /// A tab with no optional fields set.
    pub fn new(id: impl Into<String>, title: impl Into<String>, tab_type: TabType) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tab_type,
            is_active: false,
            has_connected: None,
            is_modified: None,
            icon: None,
            pid: None,
            session_nonce: None,
            connection_header_pinned: None,
            duplicate_base_title: None,
            auto_title: None,
            shell_name: None,
            icon_style: None,
            distro_id: None,
            quick_connect: None,
            quick_connect_auth: None,
            settings_section: None,
            settings_request: None,
            connection: None,
            remote_file: None,
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

/// The distribution a tab reported, when the value is one this build knows.
pub(crate) fn tab_distro_id(tab: &Tab) -> Option<DistroId> {
    let value = tab.distro_id.as_deref()?;
    crate::distro::DISTRO_IDS
        .into_iter()
        .find(|distro| distro.as_str() == value)
}
