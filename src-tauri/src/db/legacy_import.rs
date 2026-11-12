//! One-time import of the JSON files that held app data before `tterm.db`.
//!
//! The import runs in one transaction together with the marker that records
//! it, so it either happens completely or is retried on the next launch.
//! Imported files are renamed to `<name>.migrated` rather than deleted.

use crate::profiles::types::{ProfileGroup, SavedProfile};
use crate::tunnel::types::TunnelRule;
use rusqlite::{Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;
use tterm_core::config::paths::ensure_config_dir;
use tterm_core::db::{Database, IMPORT_ERROR, get, init, read, sql_error};
use tterm_core::sftp::store::SftpDirectoryStore;
use tterm_core::ssh::store::KnownHostStore;

const IMPORTED_MARKER: &str = "legacy_json_imported_at";

/// Files whose contents now live in the database.
pub(crate) const IMPORTED_FILES: &[&str] = &[
    "profiles.json",
    "profile_groups.json",
    "tunnels.json",
    "ssh_known_hosts.json",
    "sftp_directories.json",
];

/// When the JSON files were imported (unix ms); `None` before that.
pub(crate) fn imported_at(connection: &Connection) -> Result<Option<i64>, String> {
    tterm_core::db::meta::get(connection, IMPORTED_MARKER)
}

/// Whether this database already records the JSON import.
pub(crate) fn db_records_import(database: &Database) -> Result<bool, String> {
    database.read(is_imported)
}

fn is_imported(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM app_meta WHERE key = ?1",
            [IMPORTED_MARKER],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error("Failed to read database metadata"))
}

/// Imports the JSON files in `config_dir` unless that already happened.
/// Returns whether an import ran.
pub(crate) fn import_json_files(database: &Database, config_dir: &Path) -> Result<bool, String> {
    let imported = database.write(|transaction| {
        if is_imported(transaction)? {
            return Ok(false);
        }
        import_files(transaction, config_dir)?;
        transaction
            .execute(
                "INSERT INTO app_meta (key, value) VALUES (?1, CAST(unixepoch('subsec') * 1000 AS INTEGER))",
                [IMPORTED_MARKER],
            )
            .map_err(sql_error("Failed to record the JSON import"))?;
        Ok(true)
    })?;

    if imported {
        for name in IMPORTED_FILES {
            let path = config_dir.join(name);
            if path.exists() {
                let target = config_dir.join(format!("{name}.migrated"));
                if let Err(error) = fs::rename(&path, &target) {
                    eprintln!(
                        "Imported '{}' into the database but could not rename it: {error}",
                        path.display()
                    );
                }
            }
        }
    }
    Ok(imported)
}

/// Moves the settings files and state that older versions kept beside
/// `config.json` into the database. Each file is done on its own; one that
/// fails stays in place and is tried again on the next launch.
pub(crate) fn import_settings_files_into(
    database: &Database,
    config_dir: &Path,
) -> Result<(), String> {
    let errors: Vec<String> = [
        crate::app_state::import_from_config_file(database, config_dir),
        crate::session::import_file(database, config_dir),
        crate::config::window_background::import_file(database, config_dir),
        crate::backup::automatic::import_settings_files(database, config_dir),
        crate::sync::import_settings_file(database, config_dir),
    ]
    .into_iter()
    .filter_map(Result::err)
    .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn import_files(connection: &Connection, config_dir: &Path) -> Result<(), String> {
    let profiles = read_json::<Vec<SavedProfile>>(&config_dir.join("profiles.json"))?;
    if let Some(profiles) = &profiles {
        crate::profiles::storage::replace_profiles(connection, profiles)?;
    }
    let groups = read_json::<Vec<ProfileGroup>>(&config_dir.join("profile_groups.json"))?;
    if groups.is_some() || profiles.is_some() {
        // These files name groups; a profile's group name is its group's id.
        let groups = crate::profiles::groups::with_legacy_groups(
            groups.unwrap_or_default(),
            profiles.as_deref().unwrap_or_default(),
        );
        crate::profiles::groups::replace_groups(connection, &groups)?;
    }
    if let Some(tunnels) = read_json::<Vec<TunnelRule>>(&config_dir.join("tunnels.json"))? {
        crate::tunnel::storage::replace_tunnels(connection, &tunnels)?;
    }
    if let Some(store) = read_json::<KnownHostStore>(&config_dir.join("ssh_known_hosts.json"))? {
        tterm_core::ssh::store::replace_known_hosts(connection, &store)?;
    }
    if let Some(store) = read_json::<SftpDirectoryStore>(&config_dir.join("sftp_directories.json"))?
    {
        tterm_core::sftp::store::replace_sftp_directories(connection, &store)?;
    }
    Ok(())
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    serde_json::from_str(&content).map(Some).map_err(|error| {
        format!(
            "Failed to import '{}' into the database: {error}. Fix or remove the file and restart tTerm.",
            path.display()
        )
    })
}

/// Whether the legacy JSON files have been imported. False when the database
/// is unavailable.
pub(crate) fn legacy_json_imported() -> bool {
    init().and_then(db_records_import).unwrap_or(false)
}

/// Moves the settings files and state older versions kept beside
/// `config.json` into the database.
pub(crate) fn import_settings_files() -> Result<(), String> {
    import_settings_files_into(get()?, &ensure_config_dir()?)
}

/// When the legacy JSON files were imported (unix ms); `None` before that
/// or when the database is unavailable.
pub(crate) fn legacy_json_imported_at() -> Option<i64> {
    read(imported_at).ok().flatten()
}

/// Imports the JSON files that held app data before the database, once.
pub(crate) fn import_legacy_json_files() -> Result<(), String> {
    let database = init()?;
    let result = ensure_config_dir().and_then(|directory| import_json_files(database, &directory));
    match result {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = IMPORT_ERROR.set(error.clone());
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
