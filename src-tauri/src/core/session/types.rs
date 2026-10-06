use crate::core::state::SessionKind;
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct TerminalShellConfig {
    #[cfg(target_os = "windows")]
    pub shell: String,
    #[cfg(target_os = "windows")]
    pub custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    pub custom_args: Option<String>,
    /// Directory the shell last reported; the next start resumes there when
    /// it still exists.
    pub cwd: Option<String>,
}

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
    /// Authenticate through the local SSH agent instead of a password or key file.
    pub use_agent: bool,
    /// Authenticate by answering the server's prompts; nothing is stored.
    pub keyboard_interactive: bool,
    /// No method configured: try the agent, key files, then prompts.
    pub auto_auth: bool,
}

impl JumpHostPlan {
    pub fn auth_method(&self) -> crate::ssh::AuthMethod<'_> {
        crate::ssh::AuthMethod::from_plan(
            self.auto_auth,
            self.use_agent,
            self.keyboard_interactive,
            self.private_key_path.as_deref(),
            self.private_key_passphrase.as_deref(),
            self.password.as_deref(),
        )
    }
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

/// Secret key of a profile's dedicated sudo password, kept apart from the
/// login password so key- and agent-authenticated profiles can have one.
pub fn sudo_secret_key(profile_id: &str) -> String {
    format!("{profile_id}:sudo")
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
    #[serde(default, alias = "authMethod")]
    pub auth_method: Option<String>,
    /// Forward the local SSH agent to the target host, whatever `auth_method` is.
    #[serde(default, alias = "agentForward")]
    pub agent_forward: bool,
    /// Ordered jump host chain to tunnel through before reaching the target.
    pub jump_hosts: Vec<JumpHostOptions>,
    /// SSH: charset label of the remote shell (`gbk`, `big5`, ...); UTF-8 when unset.
    pub encoding: Option<String>,
    /// SSH: install tTerm's shell integration on the host so its shell marks commands.
    pub shell_integration: bool,
    #[cfg(target_os = "windows")]
    pub terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    pub terminal_shell_custom_args: Option<String>,
    /// Local terminal: directory to start in, as last reported by the shell.
    pub cwd: Option<String>,
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
    #[serde(default, alias = "authMethod")]
    auth_method: Option<String>,
    #[serde(default, alias = "agentForward")]
    agent_forward: bool,
    #[serde(default, alias = "jumpHost")]
    legacy_jump_host: Option<JumpHostOptions>,
    #[serde(default, alias = "jumpHosts")]
    jump_hosts: Vec<JumpHostOptions>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default, alias = "shellIntegration")]
    shell_integration: bool,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShell")]
    terminal_shell: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomPath")]
    terminal_shell_custom_path: Option<String>,
    #[cfg(target_os = "windows")]
    #[serde(default, alias = "terminalShellCustomArgs")]
    terminal_shell_custom_args: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
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
            auth_method: raw.auth_method,
            agent_forward: raw.agent_forward,
            jump_hosts,
            encoding: raw.encoding,
            shell_integration: raw.shell_integration,
            #[cfg(target_os = "windows")]
            terminal_shell: raw.terminal_shell,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: raw.terminal_shell_custom_path,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: raw.terminal_shell_custom_args,
            cwd: raw.cwd,
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
            auth_method: None,
            agent_forward: false,
            jump_hosts: Vec::new(),
            encoding: None,
            shell_integration: false,
            #[cfg(target_os = "windows")]
            terminal_shell: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_path: None,
            #[cfg(target_os = "windows")]
            terminal_shell_custom_args: None,
            cwd: None,
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
    /// Automatically re-establish this session after a recoverable disconnect.
    /// Only meaningful for SSH sessions.
    pub reconnect_enabled: bool,
    /// How many automatic reconnect attempts to make before giving up and
    /// reporting the disconnect. Only meaningful for SSH sessions.
    pub reconnect_max_attempts: u32,
    pub private_key_path: Option<String>,
    pub private_key_passphrase: Option<String>,
    /// Authenticate through the local SSH agent instead of a password or key file.
    pub use_agent: bool,
    /// Authenticate by answering the server's prompts; nothing is stored.
    pub keyboard_interactive: bool,
    /// No method configured (quick connect): try the agent, key files, then
    /// prompts. `private_key_path` is then only the first key file to try.
    pub auto_auth: bool,
    /// Forward the local SSH agent to the target host so the user's keys can
    /// be used for further SSH hops from there, whatever the auth method.
    /// The target's `SshClientHandler` only bridges a server-opened
    /// forwarding channel when this is set.
    pub agent_forward: bool,
    pub terminal_shell: Option<TerminalShellConfig>,
    /// Ordered resolved jump host chain; empty means direct connection.
    pub jump_hosts: Vec<JumpHostPlan>,
    /// Charset of the remote shell; output is decoded and input encoded with it.
    pub encoding: crate::terminal::TerminalEncoding,
    /// SSH: start the remote shell with tTerm's shell integration (command
    /// marks) when the host supports it; see `crate::ssh::shell_integration`.
    pub shell_integration: bool,
}

impl SessionPlan {
    /// How to authenticate to the target host.
    pub fn auth_method(&self) -> crate::ssh::AuthMethod<'_> {
        crate::ssh::AuthMethod::from_plan(
            self.auto_auth,
            self.use_agent,
            self.keyboard_interactive,
            self.private_key_path.as_deref(),
            self.private_key_passphrase.as_deref(),
            self.password.as_deref(),
        )
    }
}
