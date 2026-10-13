//! Writing an imported backup's data into this device's settings and database.

use super::archive::validate_relative_path;
use super::automatic::restore_automatic_backup_settings;
use super::export::secret_travels;
use super::remote;
use super::{BackupImportOptions, BackupLogFile, BackupPayload, BackupSelection, MissingKeyFile};
use crate::command_library::{models::SavedCommand, repository::CommandRepository};
use crate::config::{self, AppConfig};
use crate::profiles::types::SavedProfile;
use crate::proxy::ProxyServer;
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::secret_store::{crypto::SecretKey as DataKey, store::put_secret};
use crate::ssh::store::KnownHostStore;
use crate::tunnel::types::TunnelRule;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Whether importing hands the WebDAV settings to another server or backup
/// password, which scheduled uploads and sync would then send saved
/// passwords to.
pub(super) fn import_redirects_webdav(
    payload: &BackupPayload,
    selection: &BackupSelection,
) -> Result<bool, String> {
    let Some(settings) = payload
        .webdav_backup
        .as_ref()
        .filter(|_| selection.settings)
    else {
        return Ok(false);
    };
    let replaces_backup_password = selection.secrets
        && payload
            .secrets
            .iter()
            .any(|secret| remote::SECRET_KEYS.contains(&secret.key.as_str()));
    Ok(replaces_backup_password || remote::restore_changes_account(settings)?)
}

pub(super) fn ensure_selection_available(
    requested: &BackupSelection,
    available: &BackupSelection,
) -> Result<(), String> {
    let missing = (requested.settings && !available.settings)
        || (requested.profiles && !available.profiles)
        || (requested.session && !available.session)
        || (requested.known_hosts && !available.known_hosts)
        || (requested.sftp_directories && !available.sftp_directories)
        || (requested.command_library && !available.command_library)
        || (requested.themes && !available.themes)
        || (requested.secrets && !available.secrets)
        || (requested.logs && !available.logs);
    if missing {
        return Err(
            "The backup does not contain one or more selected data categories.".to_string(),
        );
    }
    Ok(())
}

pub(super) fn missing_key_files(profiles: &[SavedProfile]) -> Vec<MissingKeyFile> {
    let mut missing = BTreeMap::<&str, Vec<String>>::new();
    for profile in profiles.iter().filter(|p| p.connection_type == "ssh") {
        // A key's certificate is a file the backup does not carry either.
        let own = (profile.auth_method.as_deref() == Some("key"))
            .then_some([&profile.private_key_path, &profile.certificate_path])
            .into_iter()
            .flatten()
            .filter_map(Option::as_deref);
        let jumps = profile
            .jump_hosts
            .iter()
            .filter(|jump| profile.uses_jump_host() && jump.auth_method == "key")
            .flat_map(|jump| [&jump.private_key_path, &jump.certificate_path])
            .filter_map(Option::as_deref);
        for path in own.chain(jumps).map(str::trim) {
            if path.is_empty() || Path::new(path).is_file() {
                continue;
            }
            let users = missing.entry(path).or_default();
            if !users.contains(&profile.name) {
                users.push(profile.name.clone());
            }
        }
    }
    missing
        .into_iter()
        .map(|(path, profiles)| MissingKeyFile {
            path: path.to_string(),
            profiles,
        })
        .collect()
}

/// Whether an imported folder setting can be used here: empty means the
/// default folder, which every device has.
pub(super) fn is_default_or_existing_directory(value: &str) -> bool {
    let directory = Path::new(value.trim());
    directory.as_os_str().is_empty() || (directory.is_absolute() && directory.is_dir())
}

/// Keeps what belongs to this device in imported settings: how passwords are
/// unlocked, and folders and a custom shell that do not exist here.
pub(super) fn keep_device_settings(imported: &mut AppConfig, current: AppConfig) {
    imported.secret_storage_mode = current.secret_storage_mode;
    imported.secret_vault_enabled = current.secret_vault_enabled;
    imported.prompt_unlock_vault_on_startup = current.prompt_unlock_vault_on_startup;
    if !is_default_or_existing_directory(&imported.terminal_log_directory) {
        imported.terminal_log_directory = current.terminal_log_directory;
    }
    if !is_default_or_existing_directory(&imported.zmodem_download_directory) {
        imported.zmodem_download_directory = current.zmodem_download_directory;
    }
    imported
        .agent_config_dirs
        .retain(|_, dir| is_default_or_existing_directory(dir));
    for (agent, dir) in current.agent_config_dirs {
        imported.agent_config_dirs.entry(agent).or_insert(dir);
    }
    // A bare command name is looked up on the PATH and may well exist here.
    let shell = Path::new(imported.terminal_shell_custom_path.trim());
    if shell.is_absolute() && !shell.is_file() {
        if imported.terminal_shell == "custom" {
            imported.terminal_shell = current.terminal_shell;
        }
        imported.terminal_shell_custom_path = current.terminal_shell_custom_path;
        imported.terminal_shell_custom_args = current.terminal_shell_custom_args;
    }
}

pub(super) fn apply_payload(
    payload: &BackupPayload,
    options: &BackupImportOptions,
    data_key: Option<&DataKey>,
) -> Result<(usize, usize, usize), String> {
    if options.selection.settings {
        if let Some(value) = payload.config.as_ref() {
            let mut imported = serde_json::from_value::<AppConfig>(value.clone())
                .map_err(|error| format!("Invalid settings: {error}"))?;
            keep_device_settings(&mut imported, config::load_config_file()?);
            config::save_config_file(&imported)?;
        }
    }

    // The WebDAV passwords are only taken together with their server.
    let webdav_restored = options.selection.settings && payload.webdav_backup.is_some();
    let (profiles_imported, secrets_imported) = crate::db::write(|transaction| {
        let profiles_imported = apply_database_payload(transaction, payload, options)?;
        let secrets_imported = match data_key {
            Some(data_key) => {
                let mut imported = 0;
                for secret in &payload.secrets {
                    if !secret_travels(&secret.key, webdav_restored) {
                        continue;
                    }
                    put_secret(transaction, data_key, &secret.key, &secret.password)?;
                    imported += 1;
                }
                imported
            }
            None => 0,
        };
        Ok((profiles_imported, secrets_imported))
    })?;
    if options.selection.logs {
        restore_log_files(&payload.logs)?;
    }

    let commands_imported = if options.selection.command_library {
        let incoming = payload.commands.as_deref().unwrap_or(&[]);
        if options.conflict_strategy == "replace" {
            replace_commands(incoming)?;
        } else {
            let repository = CommandRepository::new(crate::db::get()?);
            for command in incoming {
                repository.save(command)?;
            }
        }
        incoming.len()
    } else {
        0
    };
    Ok((profiles_imported, commands_imported, secrets_imported))
}

/// Applies the categories stored in the database. Returns how many profiles
/// the backup carried.
pub(super) fn apply_database_payload(
    transaction: &rusqlite::Connection,
    payload: &BackupPayload,
    options: &BackupImportOptions,
) -> Result<usize, String> {
    let merge = options.conflict_strategy == "merge";
    if options.selection.settings {
        if let Some(settings) = payload.webdav_backup.clone() {
            remote::restore_settings(transaction, settings)?;
        }
        if let Some(settings) = payload.automatic_backup.clone() {
            restore_automatic_backup_settings(transaction, settings)?;
        }
        if let Some(settings) = payload.sync.as_ref() {
            crate::sync::restore_settings(transaction, settings)?;
        }
        if let Some(incoming) = payload.collapsed_profile_group_keys.as_ref() {
            let mut keys = if merge {
                crate::app_state::collapsed_profile_group_keys(transaction)?
            } else {
                Vec::new()
            };
            for key in incoming {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
            crate::app_state::set_collapsed_profile_group_keys(transaction, &keys)?;
        }
    }
    if options.selection.session {
        if let Some(session) = payload.session.clone() {
            crate::session::write_session(transaction, from_backup_value(session)?)?;
        }
    }
    let mut profiles_imported = 0;
    if options.selection.profiles {
        if let Some(incoming) = payload.profiles.as_ref() {
            let incoming_profiles = serde_json::from_value::<Vec<SavedProfile>>(incoming.clone())
                .map_err(|error| format!("Invalid profiles: {error}"))?;
            profiles_imported = incoming_profiles.len();
            let profiles = if merge {
                let safe_incoming = to_backup_value(&incoming_profiles)?;
                let existing =
                    to_backup_value(&crate::profiles::storage::list_saved_profiles(transaction)?)?;
                from_backup_value(merge_arrays_by_keys(existing, safe_incoming, &["id"])?)?
            } else {
                incoming_profiles
            };
            crate::profiles::storage::replace_profiles(transaction, &profiles)?;
        }
        if let Some(incoming) = payload.profile_groups.as_ref() {
            let groups = if merge {
                let existing = to_backup_value(
                    &crate::profiles::storage::configured_profile_groups(transaction)?,
                )?;
                merge_string_arrays(existing, incoming.clone())?
            } else {
                incoming.clone()
            };
            crate::profiles::storage::replace_profile_groups(
                transaction,
                &from_backup_value::<Vec<String>>(groups)?,
            )?;
        }
        // Backups made before tunnels existed carry none; keep the current rules then.
        if let Some(incoming) = payload.tunnels.as_ref() {
            let tunnels = if merge {
                let existing =
                    to_backup_value(&crate::tunnel::storage::list_tunnel_rules(transaction)?)?;
                merge_arrays_by_keys(existing, incoming.clone(), &["id"])?
            } else {
                incoming.clone()
            };
            crate::tunnel::storage::replace_tunnels(
                transaction,
                &from_backup_value::<Vec<TunnelRule>>(tunnels)?,
            )?;
        }
        // Backups made before proxies existed carry none; keep the current servers then.
        if let Some(incoming) = payload.proxies.as_ref() {
            let proxies = if merge {
                let existing = to_backup_value(&crate::proxy::storage::list_proxies(transaction)?)?;
                merge_arrays_by_keys(existing, incoming.clone(), &["id"])?
            } else {
                incoming.clone()
            };
            crate::proxy::storage::replace_proxies(
                transaction,
                &from_backup_value::<Vec<ProxyServer>>(proxies)?,
            )?;
        }
    }
    if options.selection.known_hosts {
        if let Some(incoming) = payload.known_hosts.as_ref() {
            let incoming = from_backup_value::<KnownHostStore>(incoming.clone())?;
            if merge {
                for entry in &incoming.entries {
                    crate::ssh::store::merge_known_host(transaction, entry)?;
                }
            } else {
                crate::ssh::store::replace_known_hosts(transaction, &incoming)?;
            }
        }
    }
    if options.selection.themes {
        let incoming = payload
            .frontend_state
            .as_ref()
            .and_then(|state| state.get("customThemes"))
            .and_then(Value::as_array);
        if let Some(incoming) = incoming {
            // Merging into nothing still drops duplicate ids.
            let existing = if merge {
                Value::Array(crate::themes::list_themes(transaction)?)
            } else {
                Value::Null
            };
            let themes = from_backup_value::<Vec<Value>>(merge_arrays_by_keys(
                existing,
                Value::Array(incoming.clone()),
                &["id"],
            )?)?;
            crate::themes::replace_themes(transaction, &themes)?;
            crate::themes::mark_themes_imported(transaction)?;
        }
    }
    if options.selection.sftp_directories {
        if let Some(incoming) = payload.sftp_directories.as_ref() {
            let store = if merge {
                let existing =
                    to_backup_value(&crate::sftp::store::list_sftp_directories(transaction)?)?;
                merge_object_entries(existing, incoming.clone(), &["host", "port", "username"])?
            } else {
                incoming.clone()
            };
            crate::sftp::store::replace_sftp_directories(
                transaction,
                &from_backup_value::<SftpDirectoryStore>(store)?,
            )?;
        }
    }
    Ok(profiles_imported)
}

pub(super) fn to_backup_value<T: Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("Failed to serialize backup data: {error}"))
}

pub(super) fn from_backup_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid backup data: {error}"))
}

pub(super) fn replace_commands(commands: &[SavedCommand]) -> Result<(), String> {
    let repository = CommandRepository::new(crate::db::get()?);
    for existing in repository.list()? {
        repository.delete(&existing.id)?;
    }
    for command in commands {
        repository.save(command)?;
    }
    Ok(())
}

pub(super) fn read_json_value(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse '{}': {error}", path.display()))
}

pub(super) fn merge_arrays_by_keys(
    existing: Value,
    incoming: Value,
    keys: &[&str],
) -> Result<Value, String> {
    let mut existing = match existing {
        Value::Array(values) => values,
        Value::Null => Vec::new(),
        _ => return Err("Existing migration target is not an array.".to_string()),
    };
    let incoming = incoming
        .as_array()
        .ok_or_else(|| "Imported migration data is not an array.".to_string())?;
    let mut indexes = HashMap::new();
    for (index, item) in existing.iter().enumerate() {
        indexes.insert(json_key(item, keys)?, index);
    }
    for item in incoming {
        let key = json_key(item, keys)?;
        if let Some(index) = indexes.get(&key).copied() {
            existing[index] = item.clone();
        } else {
            indexes.insert(key, existing.len());
            existing.push(item.clone());
        }
    }
    Ok(Value::Array(existing))
}

pub(super) fn json_key(value: &Value, keys: &[&str]) -> Result<String, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Migration entry is not an object.".to_string())?;
    keys.iter()
        .map(|key| {
            object
                .get(*key)
                .map(Value::to_string)
                .ok_or_else(|| format!("Migration entry is missing key '{key}'."))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("\u{1f}"))
}

fn merge_object_entries(existing: Value, incoming: Value, keys: &[&str]) -> Result<Value, String> {
    let existing_entries = existing
        .as_object()
        .and_then(|object| object.get("entries"))
        .cloned()
        .unwrap_or(Value::Array(Vec::new()));
    let incoming_entries = incoming
        .as_object()
        .and_then(|object| object.get("entries"))
        .cloned()
        .ok_or_else(|| "Imported migration store has no entries array.".to_string())?;
    Ok(serde_json::json!({
        "entries": merge_arrays_by_keys(existing_entries, incoming_entries, keys)?
    }))
}

fn merge_string_arrays(existing: Value, incoming: Value) -> Result<Value, String> {
    let mut result = existing.as_array().cloned().unwrap_or_default();
    let mut seen = result
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<HashSet<_>>();
    for value in incoming
        .as_array()
        .ok_or_else(|| "Imported profile groups are not an array.".to_string())?
    {
        let group = value
            .as_str()
            .ok_or_else(|| "Imported profile group is not a string.".to_string())?;
        if seen.insert(group.to_string()) {
            result.push(Value::String(group.to_string()));
        }
    }
    Ok(Value::Array(result))
}

pub(super) fn imported_logs_root() -> Result<PathBuf, String> {
    Ok(config::paths::get_config_path()?
        .join("logs")
        .join("imported"))
}

fn restore_log_files(logs: &[BackupLogFile]) -> Result<(), String> {
    let root = imported_logs_root()?;
    for log in logs {
        validate_relative_path(&log.relative_path)?;
        let bytes = BASE64
            .decode(&log.data_b64)
            .map_err(|_| "Backup contains invalid terminal log data.".to_string())?;
        config::atomic::atomic_write_private(&root.join(Path::new(&log.relative_path)), bytes)?;
    }
    Ok(())
}
