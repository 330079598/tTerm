use crate::core::state::SessionKind;
use serde::Deserialize;

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
pub struct TerminalShellConfig {
    pub shell: String,
    pub custom_path: Option<String>,
    pub custom_args: Option<String>,
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone)]
pub struct TerminalShellConfig;

/// Jump host (bastion) connection parameters deserialized from the frontend.
#[derive(Debug, Deserialize, Clone)]
#[serde(from = "RawJumpHostOptions")]
pub struct JumpHostOptions {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default, alias = "authMethod")]
    pub auth_method: Option<String>,
    #[serde(default, alias = "privateKeyPath")]
    pub private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    pub private_key_passphrase: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct RawJumpHostOptions {
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, alias = "authMethod")]
    auth_method: Option<String>,
    #[serde(default, alias = "privateKeyPath")]
    private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    private_key_passphrase: Option<String>,
}

impl From<RawJumpHostOptions> for JumpHostOptions {
    fn from(raw: RawJumpHostOptions) -> Self {
        Self {
            host: raw.host,
            port: raw.port,
            username: raw.username,
            password: raw.password,
            auth_method: raw.auth_method,
            private_key_path: raw.private_key_path,
            private_key_passphrase: raw.private_key_passphrase,
        }
    }
}

/// Resolved jump host plan used at connection time.
#[derive(Debug, Clone)]
pub struct JumpHostPlan {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
}

pub fn jump_host_secret_key(profile_id: Option<&str>, profile_name: &str) -> String {
    format!("{}:jump", profile_id.unwrap_or(profile_name))
}

pub fn jump_host_identity_secret_key(
    profile_id: Option<&str>,
    profile_name: &str,
    host: &str,
    port: u16,
    username: &str,
) -> String {
    let profile_key = profile_id.unwrap_or(profile_name);
    format!("{profile_key}:jump:{host}:{port}:{username}")
}

pub const MAX_JUMP_HOSTS: usize = 8;

#[derive(Debug, Deserialize, Clone)]
#[serde(from = "RawPtyConnectionOptions")]
pub struct PtyConnectionOptions {
    pub connection_type: Option<String>,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ignore_saved_password: bool,
    pub remember_password: Option<bool>,
    pub keepalive_interval_secs: Option<u16>,
    pub keepalive_count_max: Option<u16>,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    /// Ordered jump host chain to tunnel through before reaching the target.
    pub jump_hosts: Vec<JumpHostOptions>,
    #[cfg(target_os = "windows")]
    pub terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_args: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub(crate) struct RawPtyConnectionOptions {
    #[serde(default, rename = "type")]
    connection_type: Option<String>,
    #[serde(default, alias = "profileId")]
    profile_id: Option<String>,
    #[serde(default, alias = "profileName")]
    profile_name: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, alias = "ignoreSavedPassword")]
    ignore_saved_password: bool,
    #[serde(default, alias = "rememberPassword")]
    remember_password: Option<bool>,
    #[serde(default, alias = "keepaliveIntervalSecs")]
    keepalive_interval_secs: Option<u16>,
    #[serde(default, alias = "keepaliveCountMax")]
    keepalive_count_max: Option<u16>,
    #[serde(default, alias = "privateKeyPath")]
    private_key_path: Option<String>,
    #[serde(default, alias = "privateKeyPassphrase")]
    private_key_passphrase: Option<String>,
    #[serde(default, alias = "jumpHost")]
    legacy_jump_host: Option<JumpHostOptions>,
    #[serde(default, alias = "jumpHosts")]
    jump_hosts: Vec<JumpHostOptions>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShell")]
    terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomPath")]
    terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomArgs")]
    terminal_shell_custom_args: Option<String>,
}

impl From<RawPtyConnectionOptions> for PtyConnectionOptions {
    fn from(raw: RawPtyConnectionOptions) -> Self {
        let jump_hosts = if raw.jump_hosts.is_empty() {
            raw.legacy_jump_host.into_iter().collect()
        } else {
            raw.jump_hosts
        };

        Self {
            connection_type: raw.connection_type,
            profile_id: raw.profile_id,
            profile_name: raw.profile_name,
            host: raw.host,
            port: raw.port,
            username: raw.username,
            password: raw.password,
            ignore_saved_password: raw.ignore_saved_password,
            remember_password: raw.remember_password,
            keepalive_interval_secs: raw.keepalive_interval_secs,
            keepalive_count_max: raw.keepalive_count_max,
            private_key_path: raw.private_key_path,
            private_key_passphrase: raw.private_key_passphrase,
            jump_hosts,
            #[cfg(target_os = "windows")]
            terminal_shell: raw.terminal_shell,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: raw.terminal_shell_custom_path,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: raw.terminal_shell_custom_args,
        }
    }
}

impl Default for PtyConnectionOptions {
    fn default() -> Self {
        Self {
            connection_type: Some("terminal".to_string()),
            profile_id: None,
            profile_name: None,
            host: None,
            port: None,
            username: None,
            password: None,
            ignore_saved_password: false,
            remember_password: None,
            keepalive_interval_secs: None,
            keepalive_count_max: None,
            private_key_path: None,
            private_key_passphrase: None,
            jump_hosts: Vec::new(),
            #[cfg(target_os = "windows")]
            terminal_shell: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionPlan {
    pub kind: SessionKind,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ignore_saved_password: bool,
    pub remember_password: bool,
    pub keepalive_interval_secs: u16,
    pub keepalive_count_max: u16,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    pub terminal_shell: Option<TerminalShellConfig>,
    /// Ordered resolved jump host chain; empty means direct connection.
    pub jump_hosts: Vec<JumpHostPlan>,
}
