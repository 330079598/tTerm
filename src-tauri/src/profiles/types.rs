use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub(super) message: String,
    pub(super) network_latency_ms: Option<u64>,
}

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
    /// The saved connection this hop logs in as; the fields above are a copy
    /// of its address, kept for display and for versions that don't know it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    /// The group the profile is filed in; ungrouped when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// The names of the group and the groups above it, from the top, joined
    /// by ` / `. Filled in when profiles are listed and never stored. A path
    /// given on save files the profile there, creating the groups it names
    /// that are missing; an empty one leaves it ungrouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_path: Option<String>,
    /// The group's name from before groups had ids, read from older
    /// profiles, backups and sync files; such a group's id is its name.
    #[serde(default, rename = "group", skip_serializing)]
    pub(crate) legacy_group: String,
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
    /// Whether this connection's sessions start with tTerm's shell
    /// integration; as the SSH shell integration setting says when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_shell_integration: Option<bool>,
    /// The checkbox from before shell integration had a setting and was off
    /// unless turned on, read from older profiles, backups and sync files.
    /// Only `true` was a choice; `false` was the default, which now follows
    /// the setting.
    #[serde(default, rename = "shell_integration", skip_serializing)]
    pub(crate) legacy_shell_integration: bool,
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
    pub tab_icon: Option<tterm_core::config::tab_icons::TabIconStyle>,
    /// The Linux distribution the host says it runs (`ubuntu`, `arch`), for its
    /// tabs' icon. Set on connecting; saving the profile keeps it while the
    /// host and port stay the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// A serial connection's port and how it is opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<crate::serial::SerialOptions>,
    /// A local terminal's shell and the directory it starts in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_terminal: Option<LocalTerminalOptions>,
}

fn default_local_shell() -> String {
    "auto".to_string()
}

/// What a saved local terminal starts: one of the shells a local tab can
/// run, in a directory of its own. The shell is kept as the tab names it, so
/// a profile synced to another device starts the same kind of shell there,
/// or fails with the bar that offers another one.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalTerminalOptions {
    /// `auto`, a shell such as `zsh`, `pwsh` or `wsl`, or `custom`.
    #[serde(default = "default_local_shell")]
    pub shell: String,
    /// The program a `custom` shell runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_args: Option<String>,
    /// The user's own shell this is. While the device still lists it, its
    /// current program and arguments are started instead of the copy above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_id: Option<String>,
    /// The distribution a `wsl` shell starts; WSL's default when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wsl_distro: Option<String>,
    /// The directory the shell starts in, where `~` is the home directory.
    /// The home directory when unset or when the directory is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_directory: Option<String>,
}

impl LocalTerminalOptions {
    /// The options with blank fields unset and the shell's own fields kept
    /// only for the shell that uses them.
    pub(crate) fn normalized(self) -> Self {
        let text = |value: Option<String>| {
            value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let shell = text(Some(self.shell)).unwrap_or_else(default_local_shell);
        let custom = shell == "custom";
        Self {
            custom_path: text(self.custom_path).filter(|_| custom),
            custom_args: text(self.custom_args).filter(|_| custom),
            shell_id: text(self.shell_id).filter(|_| custom),
            wsl_distro: text(self.wsl_distro).filter(|_| shell == "wsl"),
            start_directory: text(self.start_directory),
            shell,
        }
    }
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

/// A group of saved profiles. Groups nest: a group without a parent sits at
/// the top.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "StoredProfileGroup")]
pub struct ProfileGroup {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// A group as read from a backup or a file of an older version, which held
/// groups as bare names; such a group's id is its name.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredProfileGroup {
    Name(String),
    #[serde(rename_all = "camelCase")]
    Group {
        id: String,
        name: String,
        #[serde(default)]
        parent_id: Option<String>,
    },
}

impl From<StoredProfileGroup> for ProfileGroup {
    fn from(stored: StoredProfileGroup) -> Self {
        match stored {
            StoredProfileGroup::Name(name) => Self {
                id: name.trim().to_string(),
                name: name.trim().to_string(),
                parent_id: None,
            },
            StoredProfileGroup::Group {
                id,
                name,
                parent_id,
            } => Self {
                id,
                name,
                parent_id,
            },
        }
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
    /// The path of the group the hosts are filed in, as `a / b`.
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

#[cfg(test)]
mod tests {
    use super::{LocalTerminalOptions, SavedProfile};

    #[test]
    fn a_local_terminal_profile_reads_back_its_shell_and_directory() {
        let profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": "a", "name": "web", "connection_type": "terminal",
            "local_terminal": { "shell": "wsl", "wslDistro": "Ubuntu", "startDirectory": "~/web" }
        }))
        .expect("profile");
        let local = profile.local_terminal.clone().expect("local terminal");
        assert_eq!(local.shell, "wsl");
        assert_eq!(local.wsl_distro.as_deref(), Some("Ubuntu"));
        assert_eq!(local.start_directory.as_deref(), Some("~/web"));

        let json = serde_json::to_value(&profile).expect("serialize");
        assert_eq!(json["local_terminal"]["startDirectory"], "~/web");
        assert!(json["local_terminal"].get("customPath").is_none());
    }

    #[test]
    fn normalizing_keeps_only_what_the_shell_uses() {
        let options = LocalTerminalOptions {
            shell: " ".to_string(),
            custom_path: Some("/bin/fish".to_string()),
            custom_args: None,
            shell_id: Some("id".to_string()),
            wsl_distro: Some("Ubuntu".to_string()),
            start_directory: Some("  ".to_string()),
        }
        .normalized();
        assert_eq!(options.shell, "auto");
        assert_eq!(options.custom_path, None);
        assert_eq!(options.shell_id, None);
        assert_eq!(options.wsl_distro, None);
        assert_eq!(options.start_directory, None);

        let custom = LocalTerminalOptions {
            shell: "custom".to_string(),
            custom_path: Some(" /bin/fish ".to_string()),
            custom_args: Some("-l".to_string()),
            shell_id: Some("id".to_string()),
            wsl_distro: Some("Ubuntu".to_string()),
            start_directory: Some(" /srv/app ".to_string()),
        }
        .normalized();
        assert_eq!(custom.custom_path.as_deref(), Some("/bin/fish"));
        assert_eq!(custom.custom_args.as_deref(), Some("-l"));
        assert_eq!(custom.shell_id.as_deref(), Some("id"));
        assert_eq!(custom.wsl_distro, None);
        assert_eq!(custom.start_directory.as_deref(), Some("/srv/app"));
    }
}
