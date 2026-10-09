//! This device's data as sync collections, and merged collections written
//! back. Records are normalized through the app's own types, so the local
//! side and the base compare equal when nothing changed, while fields added
//! by a newer tTerm on another device are kept in the remote document.

use super::document::Collections;
use super::merge::{Collection, SyncRecord};
use crate::command_library::SavedCommand;
use crate::config::AppConfig;
use crate::db::sql_error;
use crate::profiles::SavedProfile;
use crate::ssh::secret_store::{delete_secret, get_secret, put_secret, DataKey};
use crate::ssh::store::{KnownHostRecord, KnownHostStore};
use crate::tunnel::TunnelRule;
use rusqlite::{params, Connection, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const PROFILES: &str = "profiles";
pub(crate) const PROFILE_GROUPS: &str = "profileGroups";
pub(crate) const TUNNELS: &str = "tunnels";
pub(crate) const SECRETS: &str = "secrets";
pub(crate) const COMMANDS: &str = "commands";
pub(crate) const KNOWN_HOSTS: &str = "knownHosts";
pub(crate) const SETTINGS: &str = "settings";
pub(crate) const THEMES: &str = "themes";

/// Where a record sits in its list; stored in the record so reordering syncs.
const POSITION: &str = "_position";
/// Fields of a saved command that change on every use and stay per device.
const COMMAND_LOCAL_FIELDS: [&str; 3] = ["useCount", "lastUsedAt", "updatedAt"];

/// Settings that mean the same on every device. Paths, fonts, shells,
/// display scale, renderer, key bindings (they differ by OS), bandwidth,
/// logging and update preferences stay per device.
pub(crate) const SYNCED_SETTINGS: &[&str] = &[
    "theme",
    "theme_follow_system",
    "theme_light",
    "theme_dark",
    "favorite_themes",
    "language",
    "font_size",
    "terminal_line_height",
    "terminal_letter_spacing",
    "cursor_style",
    "copy_on_select",
    "right_click_paste",
    "confirm_multiline_paste",
    "command_marks",
    "local_shell_integration",
    "scrollback_lines",
    "terminal_padding_left_px",
    "terminal_padding_right_px",
    "terminal_padding_bottom_px",
    "startup_session_restore_mode",
    "show_jump_host_connection_info",
    "sftp_paste_upload_enabled",
    "monitor_refresh_interval_secs",
    "monitor_visible_metrics",
    "tab_width_mode",
    "tab_standard_width",
    "reconnect_enabled",
    "reconnect_max_attempts",
    "zmodem_auto_detect_enabled",
    "sudo_prompt_patterns",
    "notifications_enabled",
    "notify_command_finished",
    "notify_command_min_secs",
    "notify_terminal_requests",
    "notify_agent_waiting",
    "notify_agent_done",
    "bell_notify",
    "toast_max_visible",
    "notify_sound",
    "bell_style",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncSelection {
    /// Profiles, their groups and port-forwarding rules.
    pub profiles: bool,
    pub secrets: bool,
    pub commands: bool,
    pub known_hosts: bool,
    pub settings: bool,
    pub themes: bool,
}

impl Default for SyncSelection {
    fn default() -> Self {
        Self {
            profiles: true,
            secrets: true,
            commands: true,
            known_hosts: true,
            settings: true,
            themes: true,
        }
    }
}

impl SyncSelection {
    /// The collections this device syncs. Themes wait until the web view
    /// handed over the ones it kept, so they are not mistaken for deleted.
    pub fn categories(&self, themes_ready: bool) -> Vec<&'static str> {
        let mut categories = Vec::new();
        if self.profiles {
            categories.extend([PROFILES, PROFILE_GROUPS, TUNNELS]);
        }
        for (enabled, category) in [
            (self.secrets, SECRETS),
            (self.commands, COMMANDS),
            (self.known_hosts, KNOWN_HOSTS),
            (self.settings, SETTINGS),
            (self.themes && themes_ready, THEMES),
        ] {
            if enabled {
                categories.push(category);
            }
        }
        categories
    }
}

/// Secrets that belong to this device's setup rather than to profiles.
pub(crate) fn is_local_secret(key: &str) -> bool {
    key.starts_with("webdav:") || key.starts_with("sync:")
}

pub(crate) fn collect(
    connection: &Connection,
    data_key: &DataKey,
    categories: &[&str],
) -> Result<Collections, String> {
    let mut collections = Collections::new();
    for &category in categories {
        let collection = match category {
            PROFILES => collect_rows(connection, "profiles", normalize_profile)?,
            TUNNELS => collect_rows(connection, "tunnels", normalize_tunnel)?,
            PROFILE_GROUPS => crate::profiles::configured_profile_groups(connection)?
                .into_iter()
                .map(|name| (name, SyncRecord::new(Value::Bool(true), None)))
                .collect(),
            SECRETS => collect_secrets(connection, data_key)?,
            COMMANDS => crate::command_library::list_commands(connection)?
                .into_iter()
                .map(|command| Ok((command.id.clone(), command_record(&command)?)))
                .collect::<Result<_, String>>()?,
            KNOWN_HOSTS => collect_known_hosts(connection)?,
            SETTINGS => collect_settings()?,
            THEMES => collect_rows(connection, "custom_themes", |value| Ok(value.clone()))?,
            other => return Err(format!("Unknown sync category '{other}'.")),
        };
        collections.insert(category.to_string(), collection);
    }
    Ok(collections)
}

fn collect_rows(
    connection: &Connection,
    table: &str,
    normalize: fn(&Value) -> Result<Value, String>,
) -> Result<Collection, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, position, data, updated_at FROM {table} ORDER BY position, rowid"
        ))
        .map_err(sql_error("Failed to read sync data"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(sql_error("Failed to read sync data"))?;
    rows.into_iter()
        .map(|(id, position, data, updated_at)| {
            let mut value: Value = serde_json::from_str(&data)
                .map_err(|error| format!("Failed to parse {table} row '{id}': {error}"))?;
            if let Value::Object(fields) = &mut value {
                fields.insert(POSITION.to_string(), position.into());
            }
            Ok((id, SyncRecord::new(normalize(&value)?, updated_at)))
        })
        .collect()
}

fn collect_secrets(connection: &Connection, data_key: &DataKey) -> Result<Collection, String> {
    let mut statement = connection
        .prepare("SELECT key, updated_at FROM secrets")
        .map_err(sql_error("Failed to read saved passwords"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(sql_error("Failed to read saved passwords"))?;
    let mut collection = Collection::new();
    for (key, updated_at) in rows {
        if is_local_secret(&key) {
            continue;
        }
        if let Some(password) = get_secret(connection, data_key, &key)? {
            collection.insert(
                key,
                SyncRecord::new(Value::String(password.to_string()), Some(updated_at)),
            );
        }
    }
    Ok(collection)
}

fn known_host_id(entry: &KnownHostRecord) -> String {
    match &entry.profile_id {
        Some(id) => format!("id:{id}@{}:{}", entry.host, entry.port),
        None => format!("name:{}@{}:{}", entry.profile_name, entry.host, entry.port),
    }
}

fn known_host_data(entry: &KnownHostRecord) -> Value {
    serde_json::json!({
        "profileId": entry.profile_id,
        "profileName": entry.profile_name,
        "host": entry.host,
        "port": entry.port,
        "algorithm": entry.algorithm,
        "fingerprint": entry.fingerprint,
    })
}

fn collect_known_hosts(connection: &Connection) -> Result<Collection, String> {
    // A later row for the same endpoint wins, as it does for lookups.
    Ok(crate::ssh::store::list_known_hosts(connection)?
        .entries
        .iter()
        .map(|entry| {
            (
                known_host_id(entry),
                SyncRecord::new(known_host_data(entry), Some(entry.trusted_at)),
            )
        })
        .collect())
}

fn collect_settings() -> Result<Collection, String> {
    let config = serde_json::to_value(crate::config::load_config_file()?)
        .map_err(|error| format!("Failed to read settings: {error}"))?;
    Ok(SYNCED_SETTINGS
        .iter()
        .filter_map(|&key| {
            config
                .get(key)
                .map(|value| (key.to_string(), SyncRecord::new(value.clone(), None)))
        })
        .collect())
}

/// Runs each record of `collection` through the app's model for `category`.
pub(crate) fn normalize(category: &str, collection: &Collection) -> Result<Collection, String> {
    collection
        .iter()
        .map(|(id, record)| {
            let data = match category {
                PROFILES => normalize_profile(&record.data)?,
                TUNNELS => normalize_tunnel(&record.data)?,
                COMMANDS => normalize_command(id, &record.data)?,
                KNOWN_HOSTS => known_host_data(&known_host_from(record)?),
                PROFILE_GROUPS => Value::Bool(true),
                SECRETS if record.data.is_string() => record.data.clone(),
                SECRETS => return Err(format!("Synced password '{id}' is invalid.")),
                THEMES if record.data.get("id").and_then(Value::as_str) == Some(id) => {
                    record.data.clone()
                }
                THEMES => return Err(format!("Synced theme '{id}' is invalid.")),
                _ => record.data.clone(),
            };
            Ok((id.clone(), SyncRecord::new(data, record.updated_at)))
        })
        .collect()
}

/// Splits the list position off a record's data.
fn take_position(value: &Value) -> (Value, i64) {
    let mut value = value.clone();
    let position = value
        .as_object_mut()
        .and_then(|fields| fields.remove(POSITION))
        .and_then(|position| position.as_i64())
        .unwrap_or(0);
    (value, position)
}

fn with_position(value: Value, position: i64) -> Value {
    match value {
        Value::Object(mut fields) => {
            fields.insert(POSITION.to_string(), position.into());
            Value::Object(fields)
        }
        other => other,
    }
}

fn normalize_profile(value: &Value) -> Result<Value, String> {
    let (data, position) = take_position(value);
    let profile = serde_json::from_value::<SavedProfile>(data)
        .map_err(|error| format!("Synced profile is invalid: {error}"))?;
    let encoded = crate::profiles::encode_profile(&profile)?;
    let value = serde_json::from_str(&encoded)
        .map_err(|error| format!("Failed to encode profile: {error}"))?;
    Ok(with_position(value, position))
}

fn normalize_tunnel(value: &Value) -> Result<Value, String> {
    let (data, position) = take_position(value);
    let rule = serde_json::from_value::<TunnelRule>(data)
        .map_err(|error| format!("Synced tunnel is invalid: {error}"))?;
    let value =
        serde_json::to_value(rule).map_err(|error| format!("Failed to encode tunnel: {error}"))?;
    Ok(with_position(value, position))
}

fn command_record(command: &SavedCommand) -> Result<SyncRecord, String> {
    let mut value = serde_json::to_value(command)
        .map_err(|error| format!("Failed to encode command: {error}"))?;
    if let Value::Object(fields) = &mut value {
        for field in COMMAND_LOCAL_FIELDS {
            fields.remove(field);
        }
    }
    Ok(SyncRecord::new(value, Some(command.updated_at)))
}

/// A synced command with this device's usage counters filled in.
fn command_from(
    id: &str,
    data: &Value,
    local: Option<&SavedCommand>,
) -> Result<SavedCommand, String> {
    let mut fields = data.as_object().cloned().unwrap_or_default();
    fields.insert(
        "useCount".to_string(),
        local.map_or(0, |command| command.use_count).into(),
    );
    fields.insert(
        "lastUsedAt".to_string(),
        local.and_then(|command| command.last_used_at).into(),
    );
    fields.insert("updatedAt".to_string(), 0.into());
    let mut command = serde_json::from_value::<SavedCommand>(Value::Object(fields))
        .map_err(|error| format!("Synced command '{id}' is invalid: {error}"))?;
    command.id = id.to_string();
    Ok(command)
}

fn normalize_command(id: &str, data: &Value) -> Result<Value, String> {
    Ok(command_record(&command_from(id, data, None)?)?.data)
}

fn known_host_from(record: &SyncRecord) -> Result<KnownHostRecord, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Synced {
        profile_id: Option<String>,
        profile_name: String,
        host: String,
        port: u16,
        algorithm: String,
        fingerprint: String,
    }
    let synced = serde_json::from_value::<Synced>(record.data.clone())
        .map_err(|error| format!("Synced known host is invalid: {error}"))?;
    Ok(KnownHostRecord {
        profile_id: synced.profile_id,
        profile_name: synced.profile_name,
        host: synced.host,
        port: synced.port,
        algorithm: synced.algorithm,
        fingerprint: synced.fingerprint,
        trusted_at: record.updated_at.unwrap_or(0),
    })
}

/// Ids in `local` that `merged` no longer has.
pub(crate) fn removed_ids(local: &Collection, merged: &Collection) -> Vec<String> {
    local
        .keys()
        .filter(|id| !merged.contains_key(*id))
        .cloned()
        .collect()
}

/// Records of `merged` that are new or differ from `local`.
fn changed<'a>(
    local: &'a Collection,
    merged: &'a Collection,
) -> impl Iterator<Item = (&'a String, &'a SyncRecord)> {
    merged
        .iter()
        .filter(|(id, record)| local.get(*id).is_none_or(|old| old.data != record.data))
}

/// Writes the database categories of `merged` (normalized) over `local`.
pub(crate) fn apply_database(
    transaction: &Transaction<'_>,
    data_key: &DataKey,
    category: &str,
    local: &Collection,
    merged: &Collection,
) -> Result<(), String> {
    let removed = removed_ids(local, merged);
    match category {
        PROFILES => {
            crate::profiles::delete_profile_rows(transaction, &removed)?;
            for (id, record) in changed(local, merged) {
                let (data, position) = take_position(&record.data);
                let mut profile = serde_json::from_value::<SavedProfile>(data)
                    .map_err(|error| format!("Synced profile is invalid: {error}"))?;
                profile.id = id.clone();
                upsert_row(
                    transaction,
                    "profiles",
                    id,
                    position,
                    &crate::profiles::encode_profile(&profile)?,
                )?;
            }
        }
        TUNNELS => {
            for id in &removed {
                crate::tunnel::delete_tunnel_rule(transaction, id)?;
            }
            for (id, record) in changed(local, merged) {
                let (data, position) = take_position(&record.data);
                let mut rule = serde_json::from_value::<TunnelRule>(data)
                    .map_err(|error| format!("Synced tunnel is invalid: {error}"))?;
                rule.id = id.clone();
                let encoded = serde_json::to_string(&rule)
                    .map_err(|error| format!("Failed to encode tunnel: {error}"))?;
                upsert_row(transaction, "tunnels", id, position, &encoded)?;
            }
        }
        PROFILE_GROUPS => {
            crate::profiles::replace_profile_groups(
                transaction,
                &merged.keys().cloned().collect::<Vec<_>>(),
            )?;
        }
        COMMANDS => {
            let current = crate::command_library::list_commands(transaction)?;
            for id in &removed {
                crate::command_library::delete_command(transaction, id)?;
            }
            for (id, record) in changed(local, merged) {
                let existing = current.iter().find(|command| &command.id == id);
                let mut command = command_from(id, &record.data, existing)?;
                command.updated_at = record
                    .updated_at
                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
                crate::command_library::save_command(transaction, &command)?;
            }
        }
        KNOWN_HOSTS => {
            let entries = merged
                .values()
                .map(known_host_from)
                .collect::<Result<Vec<_>, _>>()?;
            crate::ssh::store::replace_known_hosts(transaction, &KnownHostStore { entries })?;
        }
        THEMES => {
            for id in &removed {
                crate::themes::delete_theme(transaction, id)?;
            }
            for (_, record) in changed(local, merged) {
                let (theme, position) = take_position(&record.data);
                crate::themes::upsert_theme(transaction, &theme, position)?;
            }
        }
        SECRETS => {
            for key in removed.iter().filter(|key| !is_local_secret(key)) {
                delete_secret(transaction, key)?;
            }
            for (key, record) in changed(local, merged) {
                if is_local_secret(key) {
                    continue;
                }
                let password = record.data.as_str().unwrap_or_default();
                put_secret(transaction, data_key, key, password)?;
            }
        }
        other => return Err(format!("'{other}' is not stored in the database.")),
    }
    Ok(())
}

fn upsert_row(
    connection: &Connection,
    table: &str,
    id: &str,
    position: i64,
    data: &str,
) -> Result<(), String> {
    connection
        .execute(
            &format!(
                "INSERT INTO {table} (id, position, data) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(id) DO UPDATE SET position = excluded.position, data = excluded.data"
            ),
            params![id, position, data],
        )
        .map_err(sql_error("Failed to save synced data"))?;
    Ok(())
}

/// Writes synced settings over the current ones.
pub(crate) fn apply_settings(merged: &Collection) -> Result<(), String> {
    let mut config = serde_json::to_value(crate::config::load_config_file()?)
        .map_err(|error| format!("Failed to read settings: {error}"))?;
    let Value::Object(fields) = &mut config else {
        return Err("Settings are not an object.".to_string());
    };
    for (key, record) in merged {
        if SYNCED_SETTINGS.contains(&key.as_str()) {
            fields.insert(key.clone(), record.data.clone());
        }
    }
    let config = serde_json::from_value::<AppConfig>(config)
        .map_err(|error| format!("Synced settings are invalid: {error}"))?;
    crate::config::save_config_file(&config)
}
