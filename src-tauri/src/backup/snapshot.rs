//! What an import may change, captured first so a failed import can be undone.

use super::apply::imported_logs_root;
use super::archive::validate_relative_path;
use super::remote;
use super::{BackupPayload, BackupSelection, AUTOMATIC_SETTINGS_KEY};
use crate::config;
use crate::db::meta;
use crate::profiles::SavedProfile;
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::secret_store::{restore_secrets, snapshot_secrets, SecretRow};
use crate::ssh::store::KnownHostStore;
use crate::tunnel::TunnelRule;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Database-held data a failed import restores.
#[derive(Default)]
pub(super) struct DatabaseSnapshot {
    pub(super) profiles: Option<(Vec<SavedProfile>, Vec<String>, Vec<TunnelRule>)>,
    /// Ciphertext as stored. Taken with profiles too: replacing profiles
    /// deletes the passwords of the removed ones.
    pub(super) secrets: Option<Vec<SecretRow>>,
    pub(super) known_hosts: Option<KnownHostStore>,
    pub(super) sftp_directories: Option<SftpDirectoryStore>,
    pub(super) themes: Option<Vec<Value>>,
    /// The `app_meta` values an import may change: settings and the session.
    pub(super) meta: Vec<(&'static str, Option<String>)>,
}

/// The `app_meta` keys an import of `selection` may change.
fn imported_meta_keys(selection: &BackupSelection) -> Vec<&'static str> {
    let mut keys = Vec::new();
    if selection.settings {
        keys.extend([
            AUTOMATIC_SETTINGS_KEY,
            remote::SETTINGS_KEY,
            remote::LAST_ERROR_KEY,
            crate::sync::SETTINGS_KEY,
            crate::app_state::COLLAPSED_PROFILE_GROUP_KEYS,
        ]);
    }
    if selection.session {
        keys.push(crate::session::SESSION_KEY);
    }
    keys
}

pub(super) fn capture_database_snapshot(
    selection: &BackupSelection,
) -> Result<DatabaseSnapshot, String> {
    crate::db::read(|connection| snapshot_database(connection, selection))
}

pub(super) fn snapshot_database(
    connection: &rusqlite::Connection,
    selection: &BackupSelection,
) -> Result<DatabaseSnapshot, String> {
    Ok(DatabaseSnapshot {
        profiles: if selection.profiles {
            Some((
                crate::profiles::list_saved_profiles(connection)?,
                crate::profiles::configured_profile_groups(connection)?,
                crate::tunnel::list_tunnel_rules(connection)?,
            ))
        } else {
            None
        },
        secrets: if selection.profiles || selection.secrets {
            Some(snapshot_secrets(connection)?)
        } else {
            None
        },
        known_hosts: if selection.known_hosts {
            Some(crate::ssh::store::list_known_hosts(connection)?)
        } else {
            None
        },
        sftp_directories: if selection.sftp_directories {
            Some(crate::sftp::store::list_sftp_directories(connection)?)
        } else {
            None
        },
        themes: if selection.themes {
            Some(crate::themes::list_themes(connection)?)
        } else {
            None
        },
        meta: imported_meta_keys(selection)
            .into_iter()
            .map(|key| Ok((key, meta::get_raw(connection, key)?)))
            .collect::<Result<_, String>>()?,
    })
}

pub(super) fn restore_database_snapshot(snapshot: &DatabaseSnapshot) -> Result<(), String> {
    crate::db::write(|transaction| restore_database(transaction, snapshot))
}

pub(super) fn restore_database(
    transaction: &rusqlite::Connection,
    snapshot: &DatabaseSnapshot,
) -> Result<(), String> {
    if let Some((profiles, groups, tunnels)) = snapshot.profiles.as_ref() {
        crate::profiles::replace_profiles(transaction, profiles)?;
        crate::profiles::replace_profile_groups(transaction, groups)?;
        crate::tunnel::replace_tunnels(transaction, tunnels)?;
    }
    if let Some(secrets) = snapshot.secrets.as_ref() {
        restore_secrets(transaction, secrets)?;
    }
    if let Some(store) = snapshot.known_hosts.as_ref() {
        crate::ssh::store::replace_known_hosts(transaction, store)?;
    }
    if let Some(store) = snapshot.sftp_directories.as_ref() {
        crate::sftp::store::replace_sftp_directories(transaction, store)?;
    }
    if let Some(themes) = snapshot.themes.as_ref() {
        crate::themes::replace_themes(transaction, themes)?;
    }
    for (key, value) in &snapshot.meta {
        meta::put_raw(transaction, key, value.as_deref())?;
    }
    Ok(())
}

/// Each config file's path and its bytes before an import, `None` if it did
/// not exist.
type FileSnapshot = Vec<(PathBuf, Option<Vec<u8>>)>;

pub(super) fn capture_file_snapshot(
    selection: &BackupSelection,
    payload: &BackupPayload,
) -> Result<FileSnapshot, String> {
    let directory = config::ensure_config_dir()?;
    let mut names = Vec::new();
    if selection.settings {
        names.push("config.json");
    }
    let mut snapshot = names
        .into_iter()
        .map(|name| {
            let path = directory.join(name);
            let content =
                if path.exists() {
                    Some(fs::read(&path).map_err(|error| {
                        format!("Failed to snapshot '{}': {error}", path.display())
                    })?)
                } else {
                    None
                };
            Ok((path, content))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if selection.logs {
        let log_root = imported_logs_root()?;
        for log in &payload.logs {
            validate_relative_path(&log.relative_path)?;
            let path = log_root.join(Path::new(&log.relative_path));
            let content =
                if path.exists() {
                    Some(fs::read(&path).map_err(|error| {
                        format!("Failed to snapshot '{}': {error}", path.display())
                    })?)
                } else {
                    None
                };
            snapshot.push((path, content));
        }
    }
    Ok(snapshot)
}

pub(super) fn restore_file_snapshot(snapshot: &[(PathBuf, Option<Vec<u8>>)]) -> Result<(), String> {
    let mut errors = Vec::new();
    for (path, content) in snapshot {
        let result = match content {
            Some(bytes) => config::atomic_write(path, bytes),
            None if path.exists() => fs::remove_file(path)
                .map_err(|error| format!("Failed to remove '{}': {error}", path.display())),
            None => Ok(()),
        };
        if let Err(error) = result {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
