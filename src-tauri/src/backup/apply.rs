//! Writing an imported backup's data into this device's settings and database.

use super::archive::validate_relative_path;
use super::automatic::restore_automatic_backup_settings;
use super::export::secret_travels;
use super::remote;
use super::{BackupImportOptions, BackupLogFile, BackupPayload, BackupSelection, MissingKeyFile};
use crate::command_library::{models::SavedCommand, repository::CommandRepository};
use crate::config::local_shells::LocalShell;
use crate::config::{self, AppConfig};
use crate::profiles::groups::{list_groups, repair, replace_groups, with_legacy_groups};
use crate::profiles::storage::normalize_profile;
use crate::profiles::types::{ProfileGroup, SavedProfile};
use crate::proxy::ProxyServer;
use crate::serial::history::{store as serial_sends, SerialSend};
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::secret_store::{crypto::SecretKey as DataKey, store::put_secret};
use crate::ssh::store::KnownHostStore;
use crate::tunnel::types::TunnelRule;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
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
            if path.is_empty() || crate::ssh::key_file::key_exists(path) {
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
    keep_device_shells(imported, &current);
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
}

/// Keeps this device's shells in imported settings, with the imported ones
/// that exist here added, and its default shell when the imported one
/// doesn't exist here.
fn keep_device_shells(imported: &mut AppConfig, current: &AppConfig) {
    // A bare command name is looked up on the PATH and may well exist here.
    let exists_here = |path: &str| {
        let shell = Path::new(path.trim().trim_matches('"'));
        !shell.is_absolute() || shell.is_file()
    };
    let mut shells = current.local_shells.clone();
    for mut shell in std::mem::take(&mut imported.local_shells) {
        if !exists_here(&shell.path) {
            continue;
        }
        match shells.iter().find(|kept| kept.id == shell.id) {
            Some(kept) if same_program(kept, &shell) => continue,
            // Another program under an id this device already uses, as every
            // shell migrated by v0.7.2-beta.7 got `default`, comes in as its own.
            Some(_) => {
                let id = uuid::Uuid::new_v4().to_string();
                if imported.terminal_shell_id == shell.id {
                    imported.terminal_shell_id.clone_from(&id);
                }
                shell.id = id;
            }
            None => {}
        }
        shells.push(shell);
    }
    imported.local_shells = shells;
    imported.hidden_shells.clone_from(&current.hidden_shells);

    // A built-in default shell or WSL distribution this device doesn't have
    // can't start here.
    let distro = imported.terminal_shell_wsl_distro.trim();
    let missing_distro = imported.terminal_shell == "wsl"
        && !distro.is_empty()
        && !crate::terminal::pty::wsl_distro_installed(distro);
    if missing_distro || !crate::terminal::pty::has_terminal_shell(&imported.terminal_shell) {
        imported.terminal_shell.clone_from(&current.terminal_shell);
        imported
            .terminal_shell_custom_path
            .clone_from(&current.terminal_shell_custom_path);
        imported
            .terminal_shell_custom_args
            .clone_from(&current.terminal_shell_custom_args);
        imported
            .terminal_shell_id
            .clone_from(&current.terminal_shell_id);
        imported
            .terminal_shell_wsl_distro
            .clone_from(&current.terminal_shell_wsl_distro);
        return;
    }

    let id = &imported.terminal_shell_id;
    let listed = imported.local_shells.iter().find(|shell| &shell.id == id);
    if !exists_here(&imported.terminal_shell_custom_path) || (!id.is_empty() && listed.is_none()) {
        if imported.terminal_shell == "custom" {
            imported.terminal_shell.clone_from(&current.terminal_shell);
        }
        imported
            .terminal_shell_custom_path
            .clone_from(&current.terminal_shell_custom_path);
        imported
            .terminal_shell_custom_args
            .clone_from(&current.terminal_shell_custom_args);
        imported
            .terminal_shell_id
            .clone_from(&current.terminal_shell_id);
    } else if let Some(shell) = listed {
        imported.terminal_shell_custom_path = shell.path.clone();
        imported.terminal_shell_custom_args = shell.args.clone();
    }
}

/// Whether two of the user's shells start the same program with the same arguments.
fn same_program(a: &LocalShell, b: &LocalShell) -> bool {
    a.path.trim() == b.path.trim() && a.args.trim() == b.args.trim()
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
        // A backup made before sounds travelled keeps this device's sound.
        if let Some(sound) = payload.notification_sound.as_deref() {
            crate::notification::sound::restore(sound)?;
        }
    }

    // The WebDAV passwords are only taken together with their server.
    let webdav_restored = options.selection.settings && payload.webdav_backup.is_some();
    let (profiles_imported, secrets_imported) = crate::db::write(|transaction| {
        let profiles_imported = apply_database_payload(transaction, payload, options)?;
        if let Some(sends) = payload
            .serial_sends
            .as_deref()
            .filter(|_| options.selection.command_library)
        {
            import_serial_sends(transaction, sends, options.conflict_strategy == "merge")?;
        }
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
    // The imported tabs reuse the ids of the tabs open now, whose saved
    // content must not show in them.
    if options.selection.session && payload.session.is_some() {
        crate::session::scrollback::clear_all()?;
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
        let mut legacy_groups = Vec::new();
        if let Some(incoming) = payload.profiles.as_ref() {
            let mut incoming_profiles =
                serde_json::from_value::<Vec<SavedProfile>>(incoming.clone())
                    .map_err(|error| format!("Invalid profiles: {error}"))?;
            profiles_imported = incoming_profiles.len();
            // A backup made before groups had ids files profiles by group name.
            legacy_groups = with_legacy_groups(Vec::new(), &incoming_profiles);
            for profile in &mut incoming_profiles {
                normalize_profile(profile);
            }
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
        if payload.profile_groups.is_some() || !legacy_groups.is_empty() {
            let incoming = match payload.profile_groups.as_ref() {
                Some(groups) => from_backup_value::<Vec<ProfileGroup>>(groups.clone())?,
                None => Vec::new(),
            };
            let mut groups = if merge {
                list_groups(transaction)?
            } else {
                Vec::new()
            };
            for group in legacy_groups.into_iter().chain(incoming) {
                match groups.iter_mut().find(|existing| existing.id == group.id) {
                    Some(existing) => *existing = group,
                    None => groups.push(group),
                }
            }
            replace_groups(transaction, &groups)?;
        }
        repair(transaction)?;
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

/// Takes a backup's serial send history: instead of this device's in a
/// replace, beside it in a merge, where the later of a send made on both wins.
pub(super) fn import_serial_sends(
    transaction: &rusqlite::Connection,
    sends: &[SerialSend],
    merge: bool,
) -> Result<(), String> {
    if !merge {
        return serial_sends::replace_all(transaction, sends);
    }
    let current: HashMap<String, i64> = serial_sends::list_all(transaction)?
        .into_iter()
        .map(|send| (send.id, send.sent_at))
        .collect();
    for send in sends {
        if current
            .get(&send.id)
            .is_none_or(|sent_at| *sent_at < send.sent_at)
        {
            serial_sends::put(transaction, send)?;
        }
    }
    serial_sends::trim(transaction)
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
