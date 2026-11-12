//! Tab and context-menu value types, from `src/types/tab.ts`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::distro::DistroId;
use crate::keymap::actions::KeymapActionId;
use crate::workspace::tab_icons::TabIconStyle;

mod serde_impl;

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

/// The known shells a local tab starts; unknown saved values remain in its raw fields.
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

/// The connection fields a tab carries, including unrecognized saved fields.
#[derive(Debug, Clone, Default)]
pub struct TabConnection {
    /// The kind of connection.
    pub connection_type: Option<ConnectionType>,
    /// The saved connection the tab was opened from.
    pub profile_id: Option<String>,
    /// The saved connection's display name.
    pub profile_name: Option<String>,
    /// The remote host.
    pub host: Option<String>,
    /// The remote SSH port, when it is a valid unsigned integer.
    pub port: Option<u32>,
    /// The remote user name.
    pub username: Option<String>,
    /// A password typed for this tab.
    pub password: Option<String>,
    /// Whether a saved password is ignored.
    pub ignore_saved_password: Option<bool>,
    /// Whether the password should be remembered.
    pub remember_password: Option<bool>,
    /// The SSH keepalive interval in seconds.
    pub keepalive_interval_secs: Option<u32>,
    /// The SSH keepalive retry count.
    pub keepalive_count_max: Option<u32>,
    /// Whether the server monitor is visible.
    pub server_monitor_visible: Option<bool>,
    /// The authentication method.
    pub auth_method: Option<String>,
    /// Whether agent forwarding is enabled.
    pub agent_forward: Option<bool>,
    /// The private key path.
    pub private_key_path: Option<String>,
    /// The private key passphrase.
    pub private_key_passphrase: Option<String>,
    /// The OpenSSH certificate path.
    pub certificate_path: Option<String>,
    /// The known shell kind, when recognized.
    pub terminal_shell: Option<TerminalShellType>,
    /// A custom shell path.
    pub terminal_shell_custom_path: Option<String>,
    /// Arguments for a custom shell.
    pub terminal_shell_custom_args: Option<String>,
    /// The user's saved shell id.
    pub terminal_shell_id: Option<String>,
    /// The WSL distribution a WSL shell starts.
    pub terminal_shell_wsl_distro: Option<String>,
    /// The last directory a local shell reported.
    pub cwd: Option<String>,
    /// An AI session to resume after a restart.
    pub agent_resume: Option<Value>,
    /// The ordered SSH jump host chain.
    pub jump_hosts: Option<Value>,
    /// The remote shell's character encoding.
    pub encoding: Option<String>,
    /// The terminal color theme.
    pub terminal_theme: Option<String>,
    /// Commands typed once the shell is ready.
    pub login_script: Option<String>,
    /// Whether shell integration is requested.
    pub ssh_shell_integration: Option<bool>,
    /// The tab's session logging policy.
    pub session_log: Option<String>,
    /// Where the SFTP drawer opens.
    pub sftp_start_directory: Option<Value>,
    /// Whether SFTP works as root through sudo.
    pub sftp_sudo: Option<bool>,
    /// The keyword highlight set.
    pub highlight_set: Option<String>,
    /// The proxy used to reach the first host.
    pub proxy: Option<Value>,
    /// Serial port settings.
    pub serial: Option<Value>,
    /// Original fields, in JavaScript object enumeration order.
    pub(crate) raw_fields: Map<String, Value>,
}

/// A tab with typed accessors and lossless storage for old session fields.
#[derive(Debug, Clone)]
pub struct Tab {
    /// The tab's unique id, which pane trees refer to.
    pub id: String,
    /// The title shown in the tab bar.
    pub title: String,
    /// What the tab shows, when recognized.
    pub tab_type: TabType,
    /// Whether the tab is in front in its group.
    pub is_active: bool,
    /// Whether the tab's session has connected at least once.
    pub has_connected: Option<bool>,
    /// Whether the tab's content has unsaved changes.
    pub is_modified: Option<bool>,
    /// The tab's icon name in the web client.
    pub icon: Option<String>,
    /// The process id of a local shell, when it is a valid unsigned integer.
    pub pid: Option<u32>,
    /// The generation of the session this tab opens.
    pub session_nonce: Option<u32>,
    /// Whether the connection header stays visible; missing means pinned.
    pub connection_header_pinned: Option<bool>,
    /// The base name a duplicated tab numbers its copies from.
    pub duplicate_base_title: Option<String>,
    /// Whether the title follows the shell's directory and command.
    pub auto_title: Option<bool>,
    /// The shell a local tab runs.
    pub shell_name: Option<String>,
    /// The icon and color chosen for this tab, when structurally valid.
    pub icon_style: Option<TabIconStyle>,
    /// The reported Linux distribution, including unknown saved strings.
    pub distro_id: Option<String>,
    /// Whether this is an unsaved quick connection.
    pub quick_connect: Option<bool>,
    /// The authentication method a quick connection used.
    pub quick_connect_auth: Option<Value>,
    /// The settings section a page tab should show.
    pub settings_section: Option<String>,
    /// A counter that makes repeated settings requests observable.
    pub settings_request: Option<u32>,
    /// The saved connection the tab was opened from.
    pub connection: Option<TabConnection>,
    /// The remote file a file-editor tab shows.
    pub remote_file: Option<Value>,
    /// Original fields, in JavaScript object enumeration order.
    pub(crate) raw_fields: Map<String, Value>,
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
            raw_fields: Map::new(),
        }
    }

    /// Removes a saved field before normalization replaces or drops its value.
    pub(crate) fn forget_saved_field(&mut self, key: &str) {
        self.raw_fields.shift_remove(key);
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
