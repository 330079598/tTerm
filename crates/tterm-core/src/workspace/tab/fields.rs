//! Typed reads and setters for the canonical session fields.

use super::serde_impl::read;
use super::{ConnectionFields, Tab, TabConnection, TabFields};
use serde_json::{Map, Value};

macro_rules! optional_fields {
    ($target:ty, $view:ty, $reader:ident; $($field:ident, $setter:ident: $kind:ty => $key:literal),* $(,)?) => {
        #[doc = "Refreshes the immutable projection from canonical JSON fields."]
        pub(super) fn $reader(view: &mut $view, fields: &Map<String, Value>) {
            $(view.$field = read(fields, $key);)*
        }
        impl $target {
            $(#[doc = concat!("Replaces `", $key, "` in place, or removes it when absent.")]
            pub fn $setter(&mut self, value: Option<$kind>) {
                self.set_saved_field($key, value.and_then(|value| serde_json::to_value(value).ok()));
            })*
        }
    };
}

optional_fields!(Tab, TabFields, read_tab;
    has_connected, set_has_connected: bool => "hasConnected",
    is_modified, set_is_modified: bool => "isModified",
    icon, set_icon: String => "icon",
    pid, set_pid: u32 => "pid",
    session_nonce, set_session_nonce: u32 => "sessionNonce",
    connection_header_pinned, set_connection_header_pinned: bool => "connectionHeaderPinned",
    duplicate_base_title, set_duplicate_base_title: String => "duplicateBaseTitle",
    auto_title, set_auto_title: bool => "autoTitle",
    shell_name, set_shell_name: String => "shellName",
    icon_style, set_icon_style: crate::workspace::tab_icons::TabIconStyle => "iconStyle",
    distro_id, set_distro_id: String => "distroId",
    quick_connect, set_quick_connect: bool => "quickConnect",
    quick_connect_auth, set_quick_connect_auth: Value => "quickConnectAuth",
    settings_section, set_settings_section: String => "settingsSection",
    settings_request, set_settings_request: u32 => "settingsRequest",
    connection, set_connection: TabConnection => "connection",
    remote_file, set_remote_file: Value => "remoteFile",
);

optional_fields!(TabConnection, ConnectionFields, read_connection;
    connection_type, set_connection_type: super::ConnectionType => "type",
    profile_id, set_profile_id: String => "profileId",
    profile_name, set_profile_name: String => "profileName",
    host, set_host: String => "host",
    port, set_port: u32 => "port",
    username, set_username: String => "username",
    password, set_password: String => "password",
    ignore_saved_password, set_ignore_saved_password: bool => "ignoreSavedPassword",
    remember_password, set_remember_password: bool => "rememberPassword",
    keepalive_interval_secs, set_keepalive_interval_secs: u32 => "keepaliveIntervalSecs",
    keepalive_count_max, set_keepalive_count_max: u32 => "keepaliveCountMax",
    server_monitor_visible, set_server_monitor_visible: bool => "serverMonitorVisible",
    auth_method, set_auth_method: String => "authMethod",
    agent_forward, set_agent_forward: bool => "agentForward",
    private_key_path, set_private_key_path: String => "privateKeyPath",
    private_key_passphrase, set_private_key_passphrase: String => "privateKeyPassphrase",
    certificate_path, set_certificate_path: String => "certificatePath",
    terminal_shell, set_terminal_shell: super::TerminalShellType => "terminalShell",
    terminal_shell_custom_path, set_terminal_shell_custom_path: String => "terminalShellCustomPath",
    terminal_shell_custom_args, set_terminal_shell_custom_args: String => "terminalShellCustomArgs",
    terminal_shell_id, set_terminal_shell_id: String => "terminalShellId",
    terminal_shell_wsl_distro, set_terminal_shell_wsl_distro: String => "terminalShellWslDistro",
    cwd, set_cwd: String => "cwd",
    agent_resume, set_agent_resume: Value => "agentResume",
    jump_hosts, set_jump_hosts: Value => "jumpHosts",
    encoding, set_encoding: String => "encoding",
    terminal_theme, set_terminal_theme: String => "terminalTheme",
    login_script, set_login_script: String => "loginScript",
    ssh_shell_integration, set_ssh_shell_integration: bool => "sshShellIntegration",
    session_log, set_session_log: String => "sessionLog",
    sftp_start_directory, set_sftp_start_directory: Value => "sftpStartDirectory",
    sftp_sudo, set_sftp_sudo: bool => "sftpSudo",
    highlight_set, set_highlight_set: String => "highlightSet",
    proxy, set_proxy: Value => "proxy",
    serial, set_serial: Value => "serial",
);

impl Tab {
    /// Replaces the tab id while preserving its key position.
    pub fn set_id(&mut self, value: String) {
        self.set_saved_field("id", Some(value.into()));
    }
    /// Replaces the title while preserving its key position.
    pub fn set_title(&mut self, value: String) {
        self.set_saved_field("title", Some(value.into()));
    }
    /// Replaces the tab kind while preserving its key position.
    pub fn set_tab_type(&mut self, value: super::TabType) {
        self.set_saved_field("type", serde_json::to_value(value).ok());
    }
    /// Replaces the active flag, including malformed saved values.
    pub fn set_is_active(&mut self, value: bool) {
        self.set_saved_field("isActive", Some(value.into()));
    }
}
