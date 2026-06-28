use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_auth_method() -> String {
    "password".to_string()
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
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
    #[serde(default)]
    pub server_monitor_visible: bool,
    /// Legacy single jump host field kept only for backward-compatible reads.
    #[serde(default, rename = "jump_host", skip_serializing)]
    pub(crate) legacy_jump_host: Option<SavedJumpHost>,
    /// Ordered jump host chain used throughout the app and for all new saves.
    #[serde(default)]
    pub jump_hosts: Vec<SavedJumpHost>,
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
