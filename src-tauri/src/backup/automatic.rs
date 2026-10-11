//! Scheduled backups, recovery points and the backup history.

use super::apply::is_default_or_existing_directory;
use super::archive::build_archive;
use super::diff::value_array_len;
use super::export::collect_payload;
use super::remote;
use super::{
    AutomaticBackupSettings, BackupExportResult, BackupHistoryEntry, BackupSelection,
    AUTOMATIC_LAST_BACKUP_AT_KEY, AUTOMATIC_SETTINGS_FILE, AUTOMATIC_SETTINGS_KEY,
};
use crate::config;
use crate::db::meta;
use crate::ssh::SecretStoreState;
use chrono::Utc;
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub(super) fn run_due_automatic_backup_blocking(
    app: AppHandle,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: &SecretStoreState,
) -> Result<Option<BackupExportResult>, String> {
    let settings = load_automatic_backup_settings()?;
    validate_automatic_backup_settings(&settings)?;
    if !force && !automatic_backup_due(&settings) {
        return Ok(None);
    }
    let directory = automatic_backup_directory(&settings)?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create automatic backup directory: {error}"))?;
    let path = directory.join(format!(
        "automatic-{}.tterm-backup",
        Utc::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    let payload = collect_payload(&app, &settings.selection, frontend_state, secret_state)?;
    let profile_count = value_array_len(payload.profiles.as_ref());
    let command_count = payload.commands.as_ref().map_or(0, Vec::len);
    let archive = build_archive(&app, &settings.selection, &payload, None)?;
    config::atomic_write_private(&path, archive)?;
    crate::db::write(|transaction| {
        meta::set(
            transaction,
            AUTOMATIC_LAST_BACKUP_AT_KEY,
            &Utc::now().timestamp_millis(),
        )
    })?;
    enforce_retention(&directory, "automatic-", settings.retention_count as usize)?;
    Ok(Some(BackupExportResult {
        output_path: path.to_string_lossy().into_owned(),
        profile_count,
        command_count,
        secret_count: 0,
        encrypted: false,
    }))
}

pub(super) fn list_backup_history_blocking() -> Result<Vec<BackupHistoryEntry>, String> {
    let settings = load_automatic_backup_settings()?;
    let mut directories = vec![config::ensure_config_dir()?.join("backups")];
    let automatic = automatic_backup_directory(&settings)?;
    if !directories.contains(&automatic) {
        directories.push(automatic);
    }
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories {
        if !directory.exists() {
            continue;
        }
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("Failed to read backup history: {error}"))?
        {
            let entry = entry.map_err(|error| format!("Failed to read backup history: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("Failed to inspect backup history: {error}"))?;
            if !metadata.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("tterm-backup")
            {
                continue;
            }
            let canonical = path
                .canonicalize()
                .map_err(|error| format!("Failed to resolve backup history path: {error}"))?;
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("backup.tterm-backup")
                .to_string();
            let kind = if file_name.starts_with("automatic-") {
                "automatic"
            } else if file_name.starts_with("pre-import-") || file_name.starts_with("pre-sync-") {
                "recovery"
            } else {
                "manual"
            };
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or(0);
            entries.push(BackupHistoryEntry {
                path: canonical.to_string_lossy().into_owned(),
                file_name,
                size_bytes: metadata.len(),
                modified_at,
                kind: kind.to_string(),
            });
        }
    }
    entries.sort_by_key(|entry| Reverse(entry.modified_at));
    Ok(entries)
}

pub(super) fn delete_backup_history_entry_blocking(path: String) -> Result<bool, String> {
    let requested = PathBuf::from(path.trim());
    if requested.extension().and_then(|value| value.to_str()) != Some("tterm-backup")
        || !requested.exists()
    {
        return Ok(false);
    }
    let canonical = requested
        .canonicalize()
        .map_err(|error| format!("Failed to resolve backup path: {error}"))?;
    let settings = load_automatic_backup_settings()?;
    let allowed = [
        config::ensure_config_dir()?.join("backups"),
        automatic_backup_directory(&settings)?,
    ];
    let allowed = allowed
        .into_iter()
        .filter(|directory| directory.exists())
        .filter_map(|directory| directory.canonicalize().ok())
        .any(|directory| canonical.parent() == Some(directory.as_path()));
    if !allowed {
        return Err("Refusing to delete a backup outside managed backup directories.".to_string());
    }
    fs::remove_file(&canonical)
        .map_err(|error| format!("Failed to delete backup '{}': {error}", canonical.display()))?;
    Ok(true)
}

pub(super) fn create_pre_import_backup(
    app: &AppHandle,
    selection: &BackupSelection,
    secret_state: &SecretStoreState,
) -> Result<PathBuf, String> {
    let backup_dir = config::ensure_config_dir()?.join("backups");
    fs::create_dir_all(&backup_dir)
        .map_err(|error| format!("Failed to create backup directory: {error}"))?;
    let path = backup_dir.join(format!(
        "pre-import-{}.tterm-backup",
        Utc::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    let safe_selection = selection.without_secrets();
    let payload = collect_payload(app, &safe_selection, None, secret_state)?;
    let archive = build_archive(app, &safe_selection, &payload, None)?;
    config::atomic_write_private(&path, archive)?;
    Ok(path)
}

/// Saves a recovery point named `<prefix>-<time>.tterm-backup` in the
/// managed backup folder, without passwords, and keeps the newest `keep` of
/// that prefix. Used before a sync changes local data.
pub(crate) fn create_recovery_backup(
    app: &AppHandle,
    prefix: &str,
    frontend_state: Option<Value>,
    secret_state: &SecretStoreState,
    keep: usize,
) -> Result<PathBuf, String> {
    let backup_dir = config::ensure_config_dir()?.join("backups");
    fs::create_dir_all(&backup_dir)
        .map_err(|error| format!("Failed to create backup directory: {error}"))?;
    let path = backup_dir.join(format!(
        "{prefix}-{}.tterm-backup",
        Utc::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    let selection = BackupSelection {
        settings: true,
        profiles: true,
        known_hosts: true,
        command_library: true,
        themes: true,
        ..BackupSelection::default()
    };
    let payload = collect_payload(app, &selection, frontend_state, secret_state)?;
    let archive = build_archive(app, &selection, &payload, None)?;
    config::atomic_write_private(&path, archive)?;
    enforce_retention(&backup_dir, &format!("{prefix}-"), keep)?;
    Ok(path)
}

/// Applies the schedule a backup carries.
pub(super) fn restore_automatic_backup_settings(
    connection: &rusqlite::Connection,
    settings: AutomaticBackupSettings,
) -> Result<(), String> {
    let settings =
        adopt_automatic_backup_settings(settings, read_automatic_backup_settings(connection)?);
    validate_automatic_backup_settings(&settings)?;
    write_automatic_backup_settings(connection, &settings)
}

/// A backup's schedule as this device keeps it. Its folder is only taken
/// when it exists here; otherwise this device's folder stays.
pub(super) fn adopt_automatic_backup_settings(
    mut imported: AutomaticBackupSettings,
    current: AutomaticBackupSettings,
) -> AutomaticBackupSettings {
    if !is_default_or_existing_directory(&imported.directory) {
        imported.directory = current.directory;
    }
    imported
}

/// The saved schedule with when the last automatic backup ran.
pub(super) fn load_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    crate::db::read(read_automatic_backup_settings)
}

pub(crate) fn read_automatic_backup_settings(
    connection: &rusqlite::Connection,
) -> Result<AutomaticBackupSettings, String> {
    Ok(AutomaticBackupSettings {
        last_backup_at: meta::get(connection, AUTOMATIC_LAST_BACKUP_AT_KEY)?,
        ..meta::get(connection, AUTOMATIC_SETTINGS_KEY)?.unwrap_or_default()
    })
}

pub(super) fn write_automatic_backup_settings(
    connection: &rusqlite::Connection,
    settings: &AutomaticBackupSettings,
) -> Result<(), String> {
    meta::set(
        connection,
        AUTOMATIC_SETTINGS_KEY,
        &AutomaticBackupSettings {
            last_backup_at: None,
            ..settings.clone()
        },
    )
}

/// Moves the backup settings files of older versions into the database,
/// with when backups last ran kept apart from the settings.
pub(crate) fn import_settings_files(
    database: &crate::db::Database,
    config_dir: &Path,
) -> Result<(), String> {
    let automatic = meta::import_file(
        database,
        &config_dir.join(AUTOMATIC_SETTINGS_FILE),
        import_automatic_backup_settings,
    );
    let webdav = remote::import_settings_file(database, config_dir);
    automatic.and(webdav)
}

fn import_automatic_backup_settings(
    connection: &rusqlite::Connection,
    bytes: &[u8],
) -> Result<(), String> {
    let mut settings = meta::parse_file::<AutomaticBackupSettings>(bytes)?;
    if let Some(at) = settings.last_backup_at.take() {
        meta::set_if_absent(connection, AUTOMATIC_LAST_BACKUP_AT_KEY, &at)?;
    }
    meta::set_if_absent(connection, AUTOMATIC_SETTINGS_KEY, &settings)
}

pub(super) fn validate_automatic_backup_settings(
    settings: &AutomaticBackupSettings,
) -> Result<(), String> {
    if !matches!(settings.frequency.as_str(), "off" | "daily" | "weekly") {
        return Err("Automatic backup frequency must be off, daily, or weekly.".to_string());
    }
    if !(1..=50).contains(&settings.retention_count) {
        return Err("Automatic backup retention must be between 1 and 50.".to_string());
    }
    if settings.selection.secrets {
        return Err(
            "Automatic backups cannot include credentials because no backup password is stored. Use an encrypted manual backup for passwords."
                .to_string(),
        );
    }
    if !settings.selection.any() {
        return Err("Select at least one category for automatic backups.".to_string());
    }
    if !settings.directory.trim().is_empty() {
        let directory = Path::new(settings.directory.trim());
        if !directory.is_absolute() {
            return Err("Automatic backup directory must be an absolute path.".to_string());
        }
        if directory.exists() && !directory.is_dir() {
            return Err("Automatic backup path must be a directory.".to_string());
        }
    }
    Ok(())
}

fn automatic_backup_directory(settings: &AutomaticBackupSettings) -> Result<PathBuf, String> {
    if settings.directory.trim().is_empty() {
        Ok(config::ensure_config_dir()?.join("backups"))
    } else {
        Ok(PathBuf::from(settings.directory.trim()))
    }
}

fn automatic_backup_due(settings: &AutomaticBackupSettings) -> bool {
    let interval_ms = match settings.frequency.as_str() {
        "daily" => 24 * 60 * 60 * 1_000,
        "weekly" => 7 * 24 * 60 * 60 * 1_000,
        _ => return false,
    };
    settings
        .last_backup_at
        .is_none_or(|last| Utc::now().timestamp_millis().saturating_sub(last) >= interval_ms)
}

/// Keeps the newest `retention` backups whose names start with `prefix`.
fn enforce_retention(directory: &Path, prefix: &str, retention: usize) -> Result<(), String> {
    let mut backups = fs::read_dir(directory)
        .map_err(|error| format!("Failed to read backup directory: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(prefix) || !name.ends_with(".tterm-backup") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|(modified, _)| Reverse(*modified));
    for (_, path) in backups.into_iter().skip(retention) {
        fs::remove_file(&path).map_err(|error| {
            format!(
                "Failed to enforce automatic backup retention for '{}': {error}",
                path.display()
            )
        })?;
    }
    Ok(())
}
