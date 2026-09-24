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
    /// Forward the local SSH agent to the target host. Only meaningful when
    /// `auth_method == "agent"`.
    #[serde(default)]
    pub agent_forward: bool,
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
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
}

impl SavedProfile {
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
    pub keepalive_interval_secs: u32,
    pub keepalive_count_max: u32,
    pub jump_hosts: Vec<SavedJumpHost>,
    pub warnings: Vec<String>,
    pub unsupported_options: Vec<String>,
    /// `LocalForward` / `RemoteForward` / `DynamicForward` entries.
    pub forwards: Vec<crate::tunnel::ForwardSpec>,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub existing_profile_id: Option<String>,
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
    pub server_alive_interval: Option<u32>,
    pub server_alive_count_max: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RawSshHost {
    pub pattern: String,
    pub options: HashMap<String, Vec<String>>,
    pub unsupported_options: Vec<String>,
    pub warnings: Vec<String>,
}

pub(crate) fn default_keepalive_interval() -> u32 {
    30
}
pub(crate) fn default_keepalive_count() -> u32 {
    3
}
