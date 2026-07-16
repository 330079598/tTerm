use crate::config;
use crate::profiles;
use crate::session;
use crate::sftp;
use crate::ssh;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Manager;

const MIGRATED_CONFIG_FILES: &[&str] = &[
    "config.json",
    "profiles.json",
    "profile_groups.json",
    "session.json",
    "ssh_known_hosts.json",
    "ssh_profiles.json",
    "sftp_directories.json",
];

pub fn migrate_legacy_config_files(app: &tauri::AppHandle) -> Result<(), String> {
    let new_dir = config::ensure_config_dir()?;
    let old_dir = config::legacy_config_path()?;

    if same_path(&new_dir, &old_dir) || !old_dir.exists() {
        return Ok(());
    }

    for name in MIGRATED_CONFIG_FILES {
        let old_path = old_dir.join(name);
        if !old_path.exists() {
            continue;
        }

        let new_path = new_dir.join(name);
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create migration target dir: {}", e))?;
        }

        if new_path.exists() {
            merge_migrated_config_file(name, &old_path, &new_path)?;
            continue;
        }

        std::fs::copy(&old_path, &new_path).map_err(|e| {
            format!(
                "Failed to migrate '{}' from '{}' to '{}': {}",
                name,
                old_path.display(),
                new_path.display(),
                e
            )
        })?;
    }

    migrate_legacy_secret_vault(app)?;

    Ok(())
}

fn merge_migrated_config_file(name: &str, old_path: &Path, new_path: &Path) -> Result<(), String> {
    match name {
        "profiles.json" => merge_profiles_file(old_path, new_path),
        "profile_groups.json" => merge_profile_groups_file(old_path, new_path),
        "session.json" => merge_session_file(old_path, new_path),
        "ssh_known_hosts.json" => merge_known_hosts_file(old_path, new_path),
        "ssh_profiles.json" => merge_legacy_password_store_file(old_path, new_path),
        "sftp_directories.json" => merge_sftp_directory_store_file(old_path, new_path),
        _ => Ok(()),
    }
}

fn merge_profile_groups_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_groups = read_json_file::<Vec<String>>(old_path, "profile groups")?;
    let mut new_groups = read_json_file::<Vec<String>>(new_path, "profile groups")?;
    let mut changed = false;

    for group in old_groups {
        let group = group.trim();
        if group.is_empty() || new_groups.iter().any(|existing| existing == group) {
            continue;
        }

        new_groups.push(group.to_string());
        changed = true;
    }

    if changed {
        new_groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
        write_json_file(new_path, &new_groups, "profile groups")?;
    }

    Ok(())
}

fn merge_profiles_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_profiles = read_json_file::<Vec<profiles::SavedProfile>>(old_path, "profiles")?;
    let mut new_profiles = read_json_file::<Vec<profiles::SavedProfile>>(new_path, "profiles")?;
    let mut changed = false;

    for profile in old_profiles {
        let duplicate = new_profiles.iter().any(|existing| {
            existing.id == profile.id
                || (!profile.name.trim().is_empty() && existing.name == profile.name)
        });
        if !duplicate {
            new_profiles.push(profile);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_profiles, "profiles")?;
    }

    Ok(())
}

fn merge_session_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_session = read_json_file::<session::SessionData>(old_path, "session")?;
    let new_session = read_json_file::<session::SessionData>(new_path, "session")?;

    if old_session.last_saved > new_session.last_saved {
        write_json_file(new_path, &old_session, "session")?;
    }

    Ok(())
}

fn merge_known_hosts_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store = read_json_file::<ssh::store::KnownHostStore>(old_path, "known hosts")?;
    let mut new_store = read_json_file::<ssh::store::KnownHostStore>(new_path, "known hosts")?;
    let mut changed = false;

    for entry in old_store.entries {
        let duplicate = new_store.entries.iter().any(|existing| {
            (entry.profile_id.is_some() && existing.profile_id == entry.profile_id)
                || (existing.profile_name == entry.profile_name
                    && existing.host == entry.host
                    && existing.port == entry.port
                    && existing.algorithm == entry.algorithm
                    && existing.fingerprint == entry.fingerprint)
        });
        if !duplicate {
            new_store.entries.push(entry);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "known hosts")?;
    }

    Ok(())
}

fn merge_legacy_password_store_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store = read_json_file::<ssh::store::LegacySshPasswordStore>(
        old_path,
        "legacy SSH password store",
    )?;
    let mut new_store = read_json_file::<ssh::store::LegacySshPasswordStore>(
        new_path,
        "legacy SSH password store",
    )?;
    let mut changed = false;

    for record in old_store.profiles {
        let duplicate = new_store
            .profiles
            .iter()
            .any(|existing| existing.profile_name == record.profile_name);
        if !duplicate {
            new_store.profiles.push(record);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "legacy SSH password store")?;
    }

    Ok(())
}

fn merge_sftp_directory_store_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store =
        read_json_file::<sftp::store::SftpDirectoryStore>(old_path, "SFTP directory store")?;
    let mut new_store =
        read_json_file::<sftp::store::SftpDirectoryStore>(new_path, "SFTP directory store")?;
    let mut changed = false;

    for entry in old_store.entries {
        let duplicate = new_store.entries.iter().any(|existing| {
            existing.host == entry.host
                && existing.port == entry.port
                && existing.username == entry.username
        });
        if !duplicate {
            new_store.entries.push(entry);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "SFTP directory store")?;
    }

    Ok(())
}

fn read_json_file<T>(path: &Path, label: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} file '{}': {}", label, path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {} file '{}': {}", label, path.display(), e))
}

fn write_json_file<T>(path: &Path, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize,
{
    let content = serde_json::to_string_pretty(value).map_err(|e| {
        format!(
            "Failed to serialize {} file '{}': {}",
            label,
            path.display(),
            e
        )
    })?;
    crate::config::atomic_write(path, content)
        .map_err(|e| format!("Failed to write {} file '{}': {}", label, path.display(), e))
}

fn migrate_legacy_secret_vault(app: &tauri::AppHandle) -> Result<(), String> {
    let new_secret_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?
        .join("secrets");

    if !new_secret_dir.exists() {
        std::fs::create_dir_all(&new_secret_dir)
            .map_err(|e| format!("Failed to create secret dir: {}", e))?;
    }

    let old_secret_dir = config::legacy_config_path()?.join("secrets");
    if same_path(&new_secret_dir, &old_secret_dir) || !old_secret_dir.exists() {
        return Ok(());
    }

    for name in ["secret_vault.json", "secret_vault_config.json"] {
        let old_path = old_secret_dir.join(name);
        if !old_path.exists() {
            continue;
        }

        let new_path = new_secret_dir.join(name);
        if new_path.exists() {
            continue;
        }

        std::fs::copy(&old_path, &new_path).map_err(|e| {
            format!(
                "Failed to migrate '{}' from '{}' to '{}': {}",
                name,
                old_path.display(),
                new_path.display(),
                e
            )
        })?;
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn migrate_legacy_ssh_passwords(app: &tauri::AppHandle) -> Result<(), String> {
    let secret_state = app.state::<ssh::SecretStoreState>();
    let store = ssh::load_legacy_password_store()?;
    if store.profiles.is_empty() {
        return Ok(());
    }

    if !secret_state.keyring_available()? {
        return Ok(());
    }

    for record in store.profiles {
        if record.password.is_empty() {
            continue;
        }
        let password = secret_state.get_password(app, &record.profile_name)?;
        if password.is_none() {
            secret_state.save_password(app, &record.profile_name, &record.password)?;
        }
    }

    ssh::remove_legacy_password_store()?;
    Ok(())
}
