//! Gathering what a backup carries from this device.

use super::apply::to_backup_value;
use super::archive::{validate_payload, validate_relative_path};
use super::automatic::load_automatic_backup_settings;
use super::remote;
use super::{
    AutomaticBackupSettings, BackupExportOptions, BackupLogFile, BackupPayload, BackupSelection,
    MigrationSecretRecord,
};
use crate::command_library::CommandRepository;
use crate::config;
use crate::profiles::SavedProfile;
use crate::ssh::secret_store::get_secret;
use crate::ssh::SecretStoreState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Component, PathBuf};
use tauri::AppHandle;

pub(super) fn validate_export_options(options: &BackupExportOptions) -> Result<(), String> {
    if !options.selection.any() {
        return Err("Select at least one data category to export.".to_string());
    }
    if options.selection.secrets && !options.selection.profiles {
        return Err("Connection profiles must be included when exporting passwords.".to_string());
    }
    if options.selection.secrets
        && options
            .backup_password
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
    {
        return Err("A backup password is required when exporting saved passwords.".to_string());
    }
    if options
        .backup_password
        .as_deref()
        .is_some_and(|p| !p.is_empty() && p.chars().count() < 8)
    {
        return Err("Backup password must contain at least 8 characters.".to_string());
    }
    Ok(())
}

pub(super) fn collect_payload(
    _app: &AppHandle,
    selection: &BackupSelection,
    frontend_state: Option<Value>,
    secret_state: &SecretStoreState,
) -> Result<BackupPayload, String> {
    let data_key = if selection.secrets {
        Some(secret_state.data_key()?)
    } else {
        None
    };
    let (webdav_backup, automatic_backup, sync) = if selection.settings {
        (
            remote::settings_for_backup()?,
            // When the last backup ran stays on the device.
            Some(AutomaticBackupSettings {
                last_backup_at: None,
                ..load_automatic_backup_settings()?
            }),
            Some(crate::sync::load_settings()?),
        )
    } else {
        (None, None, None)
    };
    let mut payload = BackupPayload {
        config: read_selected_json(selection.settings, "config.json")?,
        profiles: read_selected_data(selection.profiles, crate::profiles::list_saved_profiles)?,
        profile_groups: read_selected_data(
            selection.profiles,
            crate::profiles::configured_profile_groups,
        )?,
        tunnels: read_selected_data(selection.profiles, crate::tunnel::list_tunnel_rules)?,
        session: if selection.session {
            crate::db::read(crate::session::read_session)?
                .map(|session| to_backup_value(&session))
                .transpose()?
        } else {
            None
        },
        known_hosts: read_selected_data(
            selection.known_hosts,
            crate::ssh::store::list_known_hosts,
        )?,
        sftp_directories: read_selected_data(
            selection.sftp_directories,
            crate::sftp::store::list_sftp_directories,
        )?,
        commands: if selection.command_library {
            Some(CommandRepository::new(crate::db::get()?).list()?)
        } else {
            None
        },
        frontend_state: with_saved_themes(
            selection,
            filter_frontend_state(selection, frontend_state),
        )?,
        webdav_backup,
        automatic_backup,
        sync,
        collapsed_profile_group_keys: if selection.settings {
            Some(crate::db::read(
                crate::app_state::collapsed_profile_group_keys,
            )?)
        } else {
            None
        },
        secrets: Vec::new(),
        logs: if selection.logs {
            collect_log_files()?
        } else {
            Vec::new()
        },
    };

    // Profiles and sessions are intentionally exported without inline credentials. Convert
    // through their typed models so old files containing legacy secret fields cannot leak them
    // into an otherwise encrypted or plaintext backup.
    if let Some(value) = payload.profiles.take() {
        let profiles = serde_json::from_value::<Vec<SavedProfile>>(value)
            .map_err(|error| format!("Invalid profile data: {error}"))?;
        payload.profiles = Some(
            serde_json::to_value(profiles)
                .map_err(|error| format!("Failed to sanitize profile data: {error}"))?,
        );
    }
    if let Some(value) = payload.session.take() {
        payload.session = Some(strip_sensitive_fields(value));
    }

    if let Some(data_key) = data_key.as_ref() {
        // Every saved password travels, as in a sync, so one stored under an
        // older key naming is not left behind.
        let with_webdav = payload.webdav_backup.is_some();
        let mut keys = secret_state.saved_keys()?;
        keys.retain(|key| secret_travels(key, with_webdav));
        payload.secrets = crate::db::read(|connection| {
            let mut secrets = Vec::new();
            for key in keys {
                if let Some(password) = get_secret(connection, data_key, &key)? {
                    secrets.push(MigrationSecretRecord {
                        key,
                        password: password.to_string(),
                    });
                }
            }
            Ok(secrets)
        })?;
    }
    validate_payload(&payload)?;
    Ok(payload)
}

/// Whether a backup carries the saved password `key`. The WebDAV passwords
/// go with the WebDAV settings; other secrets of a device's own setup stay.
pub(super) fn secret_travels(key: &str, with_webdav: bool) -> bool {
    if remote::SECRET_KEYS.contains(&key) {
        with_webdav
    } else {
        !crate::sync::is_local_secret(key)
    }
}

fn read_selected_data<T: Serialize>(
    selected: bool,
    read: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<Option<Value>, String> {
    if !selected {
        return Ok(None);
    }
    let data = crate::db::read(read)?;
    serde_json::to_value(data)
        .map(Some)
        .map_err(|error| format!("Failed to serialize backup data: {error}"))
}

fn read_selected_json(selected: bool, name: &str) -> Result<Option<Value>, String> {
    if !selected {
        return Ok(None);
    }
    let path = config::get_config_path()?.join(name);
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("Failed to parse '{}': {error}", path.display()))
}

/// Puts the saved themes into the web view's state, where backups have
/// always carried them. Before the web view handed its themes over to the
/// database, the ones it sent are kept.
fn with_saved_themes(
    selection: &BackupSelection,
    state: Option<Value>,
) -> Result<Option<Value>, String> {
    if !selection.themes {
        return Ok(state);
    }
    let themes = crate::db::read(|connection| {
        if crate::themes::themes_imported(connection)? {
            crate::themes::list_themes(connection).map(Some)
        } else {
            Ok(None)
        }
    })?;
    let Some(themes) = themes else {
        return Ok(state);
    };
    let mut fields = match state {
        Some(Value::Object(fields)) => fields,
        _ => serde_json::Map::new(),
    };
    fields.insert("customThemes".to_string(), Value::Array(themes));
    Ok(Some(Value::Object(fields)))
}

pub(super) fn filter_frontend_state(
    selection: &BackupSelection,
    state: Option<Value>,
) -> Option<Value> {
    let source = state?.as_object()?.clone();
    let mut filtered = serde_json::Map::new();
    if selection.themes {
        if let Some(value) = source.get("customThemes") {
            filtered.insert("customThemes".to_string(), value.clone());
        }
    }
    if selection.settings {
        for key in [
            "recentCommands",
            "recentQuickConnections",
            "sftpColumnWidths",
            "sftpView",
        ] {
            if let Some(value) = source.get(key) {
                filtered.insert(key.to_string(), value.clone());
            }
        }
    }
    (!filtered.is_empty()).then_some(Value::Object(filtered))
}

pub(super) fn strip_sensitive_fields(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(strip_sensitive_fields).collect())
        }
        Value::Object(mut object) => {
            object.retain(|key, _| {
                let normalized = key.to_ascii_lowercase();
                !normalized.contains("password")
                    && !normalized.contains("passphrase")
                    && !normalized.contains("secret")
            });
            Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| (key, strip_sensitive_fields(value)))
                    .collect(),
            )
        }
        other => other,
    }
}

fn configured_log_root() -> Result<PathBuf, String> {
    let config_value = config::load_config_file()?;
    if config_value.terminal_log_directory.trim().is_empty() {
        Ok(config::get_config_path()?.join("logs"))
    } else {
        Ok(PathBuf::from(config_value.terminal_log_directory.trim()))
    }
}

fn collect_log_files() -> Result<Vec<BackupLogFile>, String> {
    let root = configured_log_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("Failed to resolve terminal log directory: {error}"))?;
    let mut pending = vec![root.clone()];
    let mut result = Vec::new();
    let mut total_size = 0u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("Failed to read terminal log directory: {error}"))?
        {
            let entry = entry.map_err(|error| format!("Failed to read terminal log: {error}"))?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("Failed to inspect terminal log: {error}"))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if result.len() >= 5_000 {
                return Err("Terminal log export exceeds the 5,000 file limit.".to_string());
            }
            total_size = total_size.saturating_add(metadata.len());
            if total_size > 48 * 1024 * 1024 {
                return Err("Terminal log export exceeds the 48 MiB limit.".to_string());
            }
            let relative = entry
                .path()
                .strip_prefix(&root)
                .map_err(|_| "Failed to create terminal log relative path.".to_string())?
                .components()
                .map(|component| match component {
                    Component::Normal(value) => value
                        .to_str()
                        .map(str::to_string)
                        .ok_or_else(|| "Terminal log path is not valid UTF-8.".to_string()),
                    _ => Err("Terminal log path is invalid.".to_string()),
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            validate_relative_path(&relative)?;
            let bytes = fs::read(entry.path())
                .map_err(|error| format!("Failed to read terminal log: {error}"))?;
            result.push(BackupLogFile {
                relative_path: relative,
                data_b64: BASE64.encode(bytes),
            });
        }
    }
    result.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(result)
}
