//! The saved SSH connection form's model and persistence.
//!
//! The form edits the address, group path and sign-in method. It never
//! collects or writes a password: the connection asks for one when a session
//! starts. An edit starts from the stored profile, so settings the form does
//! not show, such as jump hosts and stored-credential choices, are kept.

use rusqlite::Connection;
use tterm_core::core::blocking::run_blocking;
use tterm_core::db;
use tterm_core::profiles::groups::{group_paths, list_groups};
use tterm_core::profiles::service;
use tterm_core::profiles::storage::{find_profile, get_profile, normalize_profile};
use tterm_core::profiles::types::{ProfileGroup, SavedProfile};
use tterm_core::proxy::ProxyChoice;
use tterm_core::services::AppServices;

const CONNECTION_TYPE: &str = "ssh";
const DEFAULT_PORT: u16 = 22;
// Matches the core profile defaults and `normalize_connection`'s fallback.
const DEFAULT_KEEPALIVE_INTERVAL_SECS: u32 = 15;
const DEFAULT_KEEPALIVE_COUNT_MAX: u32 = 3;
const ERR_NOT_FOUND: &str =
    "The saved connection no longer exists. Choose another saved connection.";
const ERR_NOT_SSH: &str = "Only SSH connections can be edited here.";
const ERR_UNSUPPORTED_AUTH: &str =
    "This SSH connection uses an authentication method this form cannot edit.";

/// The sign-in method the form offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthKind {
    #[default]
    Password,
    Key,
}

/// What the saved SSH connection form holds before validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDraft {
    /// The exact profile being edited; `None` creates one.
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub auth: AuthKind,
    /// The key file when `auth` is [`AuthKind::Key`]; ignored otherwise.
    pub key_path: String,
    /// Where to file the profile: group names from the top, separated by `/`.
    /// Blank leaves it ungrouped, and missing groups are created on save.
    pub group_path: String,
    /// Whether the form changed the group path. An unchanged one keeps the
    /// stored group id even if that group moved while the form was open.
    pub(super) group_changed: bool,
}

impl Default for ProfileDraft {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            host: String::new(),
            port: DEFAULT_PORT.to_string(),
            username: String::new(),
            auth: AuthKind::Password,
            key_path: String::new(),
            group_path: String::new(),
            group_changed: false,
        }
    }
}

/// One saved SSH profile as the form's fields, without reading any secret.
pub async fn load(services: &AppServices, id: String) -> Result<ProfileDraft, String> {
    let id = normalized_id(Some(id)).ok_or_else(|| ERR_NOT_FOUND.to_string())?;
    let profile = services
        .shared
        .runtime
        .spawn_blocking(move || db::read(|connection| load_profile(connection, &id)))
        .await
        .map_err(|error| format!("Failed to read the saved SSH connection: {error}"))??;
    draft_from_profile(&profile)
}

/// Validates the form and saves it through the existing profile service.
///
/// The exact id that was saved is returned. A draft without an id always
/// creates a new profile; a draft with an id updates only that profile and
/// fails when it no longer exists or is not an SSH connection.
pub async fn save(services: &AppServices, draft: ProfileDraft) -> Result<String, String> {
    let draft = NormalizedDraft::parse(draft)?;
    let runtime = services.shared.runtime.clone();
    let secrets = services.shared.secrets.clone();
    // The profile service uses the blocking worker itself. Run on the app's
    // runtime so that worker exists even when the form's task runs on GPUI's
    // executor.
    runtime
        .spawn(async move {
            let existing = match draft.id.clone() {
                Some(id) => {
                    let profile =
                        run_blocking(move || find_profile(&id))
                            .await
                            .map_err(|error| {
                                format!("Failed to read the saved SSH connection: {error}")
                            })?;
                    Some(profile.ok_or_else(|| ERR_NOT_FOUND.to_string())?)
                }
                None => None,
            };
            let profile = build_profile(existing.as_ref(), draft)?;
            let id = profile.id.clone();
            service::save_profile(&secrets, profile, None)
                .await
                .map_err(|error| format!("Failed to save the SSH connection: {error}"))?;
            Ok(id)
        })
        .await
        .map_err(|error| format!("Failed to save the SSH connection: {error}"))?
}

/// The trimmed and checked form, with the port parsed once.
#[derive(Debug)]
struct NormalizedDraft {
    id: Option<String>,
    name: String,
    host: String,
    port: u16,
    username: String,
    auth: AuthKind,
    key_path: Option<String>,
    group_path: String,
    group_changed: bool,
}

impl NormalizedDraft {
    fn parse(draft: ProfileDraft) -> Result<Self, String> {
        let host = normalize_host(&draft.host)?;
        let username = normalize_username(&draft.username)?;
        let port = normalize_port(&draft.port)?;
        let name = normalize_name(&draft.name, &username, &host);
        let key_path = match draft.auth {
            AuthKind::Password => None,
            AuthKind::Key => Some(normalize_key_path(&draft.key_path)?),
        };
        Ok(Self {
            id: normalized_id(draft.id),
            name,
            host,
            port,
            username,
            auth: draft.auth,
            key_path,
            group_path: draft.group_path.trim().to_string(),
            group_changed: draft.group_changed,
        })
    }
}

/// Builds the profile that will be saved, keeping every field the form hides.
fn build_profile(
    existing: Option<&SavedProfile>,
    draft: NormalizedDraft,
) -> Result<SavedProfile, String> {
    let existing = match (draft.id.as_deref(), existing) {
        (Some(id), Some(profile)) => {
            if profile.id != id {
                return Err(ERR_NOT_FOUND.to_string());
            }
            Some((profile, editable_auth(profile)?))
        }
        (Some(_), None) => return Err(ERR_NOT_FOUND.to_string()),
        // A draft without an id is a new profile, even if an existing one is
        // passed by mistake; it must never overwrite that profile.
        (None, _) => None,
    };

    let identity_changed = existing.is_some_and(|(profile, _)| endpoint_changed(profile, &draft));
    let distro =
        existing.and_then(|(profile, _)| profile.distro_at(Some(&draft.host), Some(draft.port)));
    let same_key = existing.is_some_and(|(profile, auth)| {
        auth == AuthKind::Key
            && profile.private_key_path.as_deref().map(str::trim) == draft.key_path.as_deref()
    });

    let mut profile = match existing {
        Some((profile, _)) => profile.clone(),
        None => new_profile(),
    };
    if let Some((stored, _)) = existing {
        // Stored credentials belong to the endpoint they were entered for.
        // Changing the host, port or user turns them off and asks the service
        // to remove them rather than trying them on the new endpoint.
        profile.remember_password =
            !identity_changed && draft.auth == AuthKind::Password && stored.remember_password;
        profile.ignore_saved_password = identity_changed || stored.ignore_saved_password;
        profile.clear_sudo_password = identity_changed;
    } else {
        profile.remember_password = false;
        profile.ignore_saved_password = false;
        profile.clear_sudo_password = false;
    }

    profile.name = draft.name;
    profile.host = Some(draft.host);
    profile.port = Some(draft.port);
    profile.username = Some(draft.username);
    profile.connection_type = CONNECTION_TYPE.to_string();
    profile.auth_method = Some(auth_method(draft.auth).to_string());
    profile.private_key_path = draft.key_path;
    profile.certificate_path = if draft.auth == AuthKind::Key && same_key {
        profile.certificate_path.take()
    } else {
        None
    };
    profile.distro = distro;
    // An unchanged group keeps the stored id, so a group that moved or was
    // renamed while the form was open still receives this profile.
    profile.group_path = draft.group_changed.then_some(draft.group_path);
    profile.password = None;
    profile.private_key_passphrase = None;
    profile.sudo_password = None;
    Ok(profile)
}

fn new_profile() -> SavedProfile {
    SavedProfile {
        id: new_profile_id(),
        name: String::new(),
        group_id: None,
        group_path: None,
        legacy_group: String::new(),
        connection_type: CONNECTION_TYPE.to_string(),
        host: None,
        port: None,
        username: None,
        password: None,
        ignore_saved_password: false,
        remember_password: false,
        auth_method: None,
        agent_forward: false,
        private_key_path: None,
        private_key_passphrase: None,
        certificate_path: None,
        keepalive_interval_secs: DEFAULT_KEEPALIVE_INTERVAL_SECS,
        keepalive_count_max: DEFAULT_KEEPALIVE_COUNT_MAX,
        server_monitor_visible: false,
        use_jump_host: Some(false),
        legacy_jump_host: None,
        jump_hosts: Vec::new(),
        sudo_autofill: true,
        sudo_password: None,
        clear_sudo_password: false,
        encoding: None,
        terminal_theme: None,
        login_script: None,
        ssh_shell_integration: None,
        legacy_shell_integration: false,
        session_log: None,
        sftp_start_directory: None,
        highlight_set: None,
        proxy: ProxyChoice::Default,
        tab_icon: None,
        distro: None,
        serial: None,
        local_terminal: None,
    }
}

/// Reads one exact profile and its full group path under the shared database lock.
fn load_profile(connection: &Connection, id: &str) -> Result<SavedProfile, String> {
    let mut profile = get_profile(connection, id)?.ok_or_else(|| ERR_NOT_FOUND.to_string())?;
    normalize_profile(&mut profile);
    let groups = list_groups(connection)?;
    profile.group_path = group_path_for(&profile, &groups);
    Ok(profile)
}

/// The path of the group a profile is filed in, including its parents.
fn group_path_for(profile: &SavedProfile, groups: &[ProfileGroup]) -> Option<String> {
    let group_id = profile
        .group_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())?;
    group_paths(groups).get(group_id).cloned()
}

fn draft_from_profile(profile: &SavedProfile) -> Result<ProfileDraft, String> {
    let auth = editable_auth(profile)?;
    Ok(ProfileDraft {
        id: Some(profile.id.clone()),
        name: profile.name.clone(),
        host: profile.host.clone().unwrap_or_default(),
        port: profile.port.unwrap_or(DEFAULT_PORT).to_string(),
        username: profile.username.clone().unwrap_or_default(),
        auth,
        key_path: match auth {
            AuthKind::Key => profile.private_key_path.clone().unwrap_or_default(),
            AuthKind::Password => String::new(),
        },
        group_path: profile.group_path.clone().unwrap_or_default(),
        group_changed: false,
    })
}

/// The two sign-in methods this form can represent; any other stored method
/// is rejected instead of being rewritten as a password.
fn editable_auth(profile: &SavedProfile) -> Result<AuthKind, String> {
    if profile.connection_type.trim() != CONNECTION_TYPE {
        return Err(ERR_NOT_SSH.to_string());
    }
    match profile.auth_method.as_deref() {
        None | Some("password") => Ok(AuthKind::Password),
        Some("key") => Ok(AuthKind::Key),
        Some(_) => Err(ERR_UNSUPPORTED_AUTH.to_string()),
    }
}

fn endpoint_changed(stored: &SavedProfile, draft: &NormalizedDraft) -> bool {
    let host = stored
        .host
        .as_deref()
        .map(|host| unbracketed(host.trim()))
        .unwrap_or_default();
    let username = stored
        .username
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    host != draft.host.as_str()
        || stored.port.unwrap_or(DEFAULT_PORT) != draft.port
        || username != draft.username.as_str()
}

fn normalize_host(raw: &str) -> Result<String, String> {
    let host = unbracketed(raw.trim());
    if host.is_empty() {
        return Err("Enter the server's host name or IP address.".to_string());
    }
    if host
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("The host name cannot contain spaces.".to_string());
    }
    if host.contains('@') {
        return Err("Enter only the host name or IP address, without a user name.".to_string());
    }
    if host.contains(['[', ']']) {
        return Err("Enter the server's host name or IP address.".to_string());
    }
    Ok(host.to_string())
}

fn normalize_username(raw: &str) -> Result<String, String> {
    let username = raw.trim();
    if username.is_empty() {
        return Err("Enter the user name.".to_string());
    }
    if username
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err("The user name cannot contain spaces.".to_string());
    }
    Ok(username.to_string())
}

fn normalize_port(raw: &str) -> Result<u16, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 5 || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Enter a port from 1 to 65535.".to_string());
    }
    let port = raw
        .parse::<u16>()
        .map_err(|_| "Enter a port from 1 to 65535.".to_string())?;
    if port == 0 {
        return Err("Enter a port from 1 to 65535.".to_string());
    }
    Ok(port)
}

fn normalize_key_path(raw: &str) -> Result<String, String> {
    let path = raw.trim();
    if path.is_empty() {
        return Err("Choose the private key file to use for key authentication.".to_string());
    }
    if path.contains(['\0', '\n', '\r']) {
        return Err("The private key path must be on one line.".to_string());
    }
    Ok(path.to_string())
}

fn normalize_name(raw: &str, username: &str, host: &str) -> String {
    let name = raw.trim();
    if name.is_empty() {
        format!("{username}@{host}")
    } else {
        name.to_string()
    }
}

fn normalized_id(id: Option<String>) -> Option<String> {
    id.map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
}

fn unbracketed(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
}

fn auth_method(auth: AuthKind) -> &'static str {
    match auth {
        AuthKind::Password => "password",
        AuthKind::Key => "key",
    }
}

/// A fresh profile id from the UUID v4 generator the other saved models use.
///
/// Core has no profile-specific id helper yet; reusing the saved local shell
/// helper keeps one id format across the database instead of adding a second.
fn new_profile_id() -> String {
    tterm_core::workspace::local_shells::saved::new_local_shell_id()
}

#[cfg(test)]
mod tests;
