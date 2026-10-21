use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_auth_method() -> String {
    "password".to_string()
}

fn default_sudo_autofill() -> bool {
    true
}

/// Jump host configuration stored as part of a saved profile.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SavedJumpHost {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    /// OpenSSH certificate for the private key; `<key>-cert.pub` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
    #[serde(default, skip_serializing)]
    pub ignore_saved_password: bool,
    #[serde(default)]
    pub remember_password: bool,
    pub auth_method: Option<String>,
    /// Forward the local SSH agent to the target host, whatever `auth_method` is.
    #[serde(default)]
    pub agent_forward: bool,
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    /// OpenSSH certificate for the private key; `<key>-cert.pub` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_path: Option<String>,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
    #[serde(default)]
    pub server_monitor_visible: bool,
    /// Whether the saved jump-host chain is used for connections. `None` preserves legacy
    /// profiles, which enabled the chain whenever at least one hop was configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_jump_host: Option<bool>,
    /// Legacy single jump host field kept only for backward-compatible reads.
    #[serde(default, rename = "jump_host", skip_serializing)]
    pub(crate) legacy_jump_host: Option<SavedJumpHost>,
    /// Ordered jump host chain used throughout the app and for all new saves.
    #[serde(default)]
    pub jump_hosts: Vec<SavedJumpHost>,
    /// Offer to fill a saved password when a sudo prompt appears.
    #[serde(default = "default_sudo_autofill")]
    pub sudo_autofill: bool,
    /// Dedicated sudo password to store; write-only, never read back.
    #[serde(default, skip_serializing)]
    pub sudo_password: Option<String>,
    /// Remove the stored sudo password on save.
    #[serde(default, skip_serializing)]
    pub clear_sudo_password: bool,
    /// Charset label of the remote shell (`gbk`, `big5`, ...); UTF-8 when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// Theme whose terminal colors this connection's tabs use; the app theme when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_theme: Option<String>,
    /// Commands typed into the remote shell once it is ready, one per line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_script: Option<String>,
    /// Install tTerm's shell integration on the host so its shell marks commands.
    #[serde(default)]
    pub shell_integration: bool,
    /// Whether this connection's sessions are logged: `always`, `never`, or
    /// as the logging setting says when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_log: Option<String>,
    /// Where this connection's SFTP drawer opens: `home`, `last` or
    /// `terminal`, or as the SFTP setting says when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sftp_start_directory: Option<String>,
    /// The keyword highlight set this connection's tabs use: a set's id, or
    /// `off`; the default set when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_set: Option<String>,
    /// Which proxy server the first host is reached through; the device's
    /// default proxy when unset.
    #[serde(default, skip_serializing_if = "crate::proxy::ProxyChoice::is_default")]
    pub proxy: crate::proxy::ProxyChoice,
    /// The icon and color this connection's tabs show; the logo of the host's
    /// distribution, once known, when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_icon: Option<crate::config::tab_icons::TabIconStyle>,
    /// The Linux distribution the host says it runs (`ubuntu`, `arch`), for its
    /// tabs' icon. Set on connecting; saving the profile keeps it while the
    /// host and port stay the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// A serial connection's port and how it is opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<crate::serial::SerialOptions>,
}

impl SavedProfile {
    /// The distribution this profile's host was seen running, while `host`
    /// and `port` still point at it.
    pub fn distro_at(&self, host: Option<&str>, port: Option<u16>) -> Option<String> {
        if self.host.as_deref() == host && self.port == port {
            self.distro.clone()
        } else {
            None
        }
    }

    pub fn uses_jump_host(&self) -> bool {
        self.use_jump_host.unwrap_or(!self.jump_hosts.is_empty())
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavedSecretSummary {
    pub key: String,
    pub profile_id: String,
    pub profile_name: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportOptions {
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub overwrite_existing: bool,
    #[serde(default)]
    pub selected_hosts: Vec<String>,
    /// Also create port-forwarding rules from the config's forward directives.
    #[serde(default)]
    pub import_forwards: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportHost {
    pub host_pattern: String,
    pub name: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub auth_method: String,
    pub private_key_path: Option<String>,
    /// `CertificateFile`, kept only with an `IdentityFile` to go with it.
    pub certificate_path: Option<String>,
    /// `ForwardAgent yes`.
    pub agent_forward: bool,
    pub keepalive_interval_secs: u32,
    pub keepalive_count_max: u32,
    pub jump_hosts: Vec<SavedJumpHost>,
    /// What `ProxyCommand` sets up, unless it became a jump host.
    pub proxy: Option<SshConfigImportProxy>,
    pub warnings: Vec<String>,
    pub unsupported_options: Vec<String>,
    /// `LocalForward` / `RemoteForward` / `DynamicForward` entries.
    pub forwards: Vec<crate::tunnel::forwards::ForwardSpec>,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub existing_profile_id: Option<String>,
}

/// The proxy an imported host connects through, from its `ProxyCommand`.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SshConfigImportProxy {
    /// `ProxyCommand none`: no proxy, even when this device has a default one.
    Direct,
    /// The HTTP or SOCKS5 proxy server the command only handed the connection to.
    Server {
        kind: crate::proxy::ProxyKind,
        host: String,
        port: u16,
    },
    /// Any other command, saved as a command proxy.
    Command { command: String },
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportPreview {
    pub source_path: String,
    pub hosts: Vec<SshConfigImportHost>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigImportResult {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub tunnels_imported: usize,
    pub tunnels_skipped: usize,
    pub profiles: Vec<SavedProfile>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SshConfigDefaults {
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub certificate_file: Option<String>,
    pub server_alive_interval: Option<u32>,
    pub server_alive_count_max: Option<u32>,
    pub forward_agent: Option<String>,
    pub proxy_command: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RawSshHost {
    pub pattern: String,
    pub options: HashMap<String, Vec<String>>,
    pub unsupported_options: Vec<String>,
    pub warnings: Vec<String>,
}

/// Matches the connection dialog's default and `normalize_connection`'s fallback.
pub(crate) fn default_keepalive_interval() -> u32 {
    15
}
pub(crate) fn default_keepalive_count() -> u32 {
    3
}
