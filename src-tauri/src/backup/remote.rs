//! Backups kept on a WebDAV server. Each upload is an ordinary
//! `.tterm-backup` archive, always encrypted with a backup password. That
//! password and the server password are kept with the other saved passwords,
//! so scheduled uploads can include credentials and need no prompt.

pub(super) mod service;

use crate::backup::BackupSelection;
use crate::backup::archive::build_archive_for_version;
use crate::backup::diff::value_array_len;
use crate::backup::export::collect_payload;
use chrono::Utc;
use files::{backup_file_name, device_name, now_ms, stale_backups, user_agent};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::sync::atomic::AtomicBool;
use tterm_core::backup::webdav::{WebDavClient, split_remote_directory};
use tterm_core::core::blocking::run_blocking;
use tterm_core::db::meta;
use tterm_core::ssh::secret_store::store::get_secret;
use tterm_core::ssh::secret_store::types::SecretStoreState;
use zeroize::Zeroizing;

mod files;

pub(crate) const SETTINGS_KEY: &str = "settings.webdav_backup";
/// Where older versions kept the settings.
const SETTINGS_FILE: &str = "webdav_backup_settings.json";
const DOWNLOAD_DIRECTORY: &str = "webdav-downloads";
const PASSWORD_KEY: &str = "webdav:password";
const BACKUP_PASSWORD_KEY: &str = "webdav:backup-password";
/// Upload history, kept apart from the settings.
const LAST_BACKUP_AT_KEY: &str = "backup.webdav.last_backup_at";
pub(crate) const LAST_ERROR_KEY: &str = "backup.webdav.last_error";
/// The saved passwords that belong to the WebDAV settings.
pub(crate) const SECRET_KEYS: [&str; 2] = [PASSWORD_KEY, BACKUP_PASSWORD_KEY];
const FILE_PREFIX: &str = "tterm-";
const FILE_EXTENSION: &str = ".tterm-backup";
const TIMESTAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
const TIMESTAMP_LEN: usize = 15;

/// Set while an upload runs, so a scheduled run and "Upload now" cannot
/// overlap and prune each other's files.
static UPLOADING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavBackupSettings {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_remote_directory")]
    pub remote_directory: String,
    #[serde(default = "default_frequency")]
    pub frequency: String,
    /// How many uploads of this device to keep; other devices' are left alone.
    #[serde(default = "default_retention_count")]
    pub retention_count: u16,
    #[serde(default = "default_selection")]
    pub selection: BackupSelection,
    /// This device's upload history, from the database. Never saved with
    /// the settings, and values sent back by the UI are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_backup_at: Option<i64>,
    /// Why the last scheduled or manual upload failed; cleared on success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for WebDavBackupSettings {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            remote_directory: default_remote_directory(),
            frequency: default_frequency(),
            retention_count: default_retention_count(),
            selection: default_selection(),
            last_backup_at: None,
            last_error: None,
        }
    }
}

fn default_remote_directory() -> String {
    "tTerm".to_string()
}

fn default_frequency() -> String {
    "daily".to_string()
}

fn default_retention_count() -> u16 {
    10
}

fn default_selection() -> BackupSelection {
    BackupSelection {
        settings: true,
        profiles: true,
        session: true,
        known_hosts: true,
        sftp_directories: true,
        command_library: true,
        themes: true,
        secrets: true,
        logs: false,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavBackupStatus {
    pub settings: WebDavBackupSettings,
    pub has_password: bool,
    pub has_backup_password: bool,
    /// Names this device's uploads; retention only prunes these.
    pub device_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavUploadResult {
    pub file_name: String,
    pub size_bytes: u64,
    pub profile_count: usize,
    pub command_count: usize,
    pub secret_count: usize,
    /// Old uploads that could not be deleted; the upload itself succeeded.
    pub prune_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavDeleteResult {
    pub deleted: Vec<String>,
    /// The first failure; the other files were still tried.
    pub error: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupEntry {
    pub file_name: String,
    pub size_bytes: u64,
    /// When the backup was made (from its name), else the server's date.
    pub created_at: Option<i64>,
    pub device: Option<String>,
    pub current_device: bool,
}

async fn run_webdav_backup_locked(
    app_version: String,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: &SecretStoreState,
) -> Result<Option<WebDavUploadResult>, String> {
    let settings = run_blocking(load_settings).await?;
    if !is_configured(&settings) {
        return if force {
            Err("Set up the WebDAV server first.".to_string())
        } else {
            Ok(None)
        };
    }
    if !force && !backup_due(&settings, now_ms()) {
        return Ok(None);
    }
    if !force && secret_state.data_key().is_err() {
        return Ok(None);
    }
    let result = upload(&app_version, &settings, frontend_state, secret_state).await;
    let message = result.as_ref().err().cloned();
    run_blocking(move || {
        tterm_core::db::write(|transaction| match message {
            Some(message) => meta::set(transaction, LAST_ERROR_KEY, &message),
            None => {
                meta::set(transaction, LAST_BACKUP_AT_KEY, &now_ms())?;
                meta::remove(transaction, LAST_ERROR_KEY)
            }
        })
    })
    .await?;
    result.map(Some)
}

async fn upload(
    app_version: &str,
    settings: &WebDavBackupSettings,
    frontend_state: Option<Value>,
    secret_state: &SecretStoreState,
) -> Result<WebDavUploadResult, String> {
    let settings = normalized_settings(settings.clone())?;
    let directory = split_remote_directory(&settings.remote_directory)?;
    let (password, archive, profile_count, command_count, secret_count) = {
        let app_version = app_version.to_string();
        let secret_state = secret_state.clone();
        let selection = settings.selection.clone();
        run_blocking(move || {
            let password = saved_secret(&secret_state, PASSWORD_KEY)?
                .ok_or_else(|| "The WebDAV password is not saved.".to_string())?;
            let backup_password = saved_secret(&secret_state, BACKUP_PASSWORD_KEY)?
                .ok_or_else(|| "The WebDAV backup password is not saved.".to_string())?;
            let payload = collect_payload(&selection, frontend_state, &secret_state)?;
            let archive = build_archive_for_version(
                &app_version,
                &selection,
                &payload,
                Some(&backup_password),
            )?;
            Ok((
                password,
                archive,
                value_array_len(payload.profiles.as_ref()),
                payload.commands.as_ref().map_or(0, Vec::len),
                payload.secrets.len(),
            ))
        })
        .await?
    };

    let client = WebDavClient::new(
        &settings.url,
        &settings.username,
        password,
        &user_agent(app_version),
    )?;
    client.ensure_directory(&directory).await?;
    let device = device_name();
    let file_name = backup_file_name(&device, Utc::now());
    let size_bytes = archive.len() as u64;
    client.put(&directory, &file_name, archive).await?;

    let prune_error = match client.list(&directory).await {
        Ok(entries) => {
            let mut errors = Vec::new();
            for name in stale_backups(&entries, &device, settings.retention_count as usize) {
                if let Err(error) = client.delete(&directory, &name).await {
                    errors.push(error);
                }
            }
            errors.first().cloned()
        }
        Err(error) => Some(error),
    };
    Ok(WebDavUploadResult {
        file_name,
        size_bytes,
        profile_count,
        command_count,
        secret_count,
        prune_error,
    })
}

/// Removes downloaded backups; also run at startup for any a quit left behind.
pub(crate) fn clear_downloads() -> Result<(), String> {
    let downloads = tterm_core::config::paths::ensure_config_dir()?.join(DOWNLOAD_DIRECTORY);
    if downloads.exists() {
        fs::remove_dir_all(&downloads)
            .map_err(|error| format!("Failed to clear old WebDAV downloads: {error}"))?;
    }
    Ok(())
}

async fn connect(
    app_version: &str,
    secret_state: &SecretStoreState,
) -> Result<(WebDavClient, Vec<String>), String> {
    let secret_state = secret_state.clone();
    let (settings, password) = run_blocking(move || {
        let settings = load_settings()?;
        if !is_configured(&settings) {
            return Err("Set up the WebDAV server first.".to_string());
        }
        let password = saved_secret(&secret_state, PASSWORD_KEY)?
            .ok_or_else(|| "The WebDAV password is not saved.".to_string())?;
        Ok((settings, password))
    })
    .await?;
    let directory = split_remote_directory(&settings.remote_directory)?;
    let client = WebDavClient::new(
        &settings.url,
        &settings.username,
        password,
        &user_agent(app_version),
    )?;
    Ok((client, directory))
}

/// The configured WebDAV folder, as the sync uses it.
pub(crate) struct SyncTarget {
    pub client: WebDavClient,
    pub directory: Vec<String>,
    /// Server, user and folder; a sync base only applies to the same target.
    pub key: String,
    /// Encrypts the sync file, like remote backups.
    pub password: Zeroizing<String>,
    pub device: String,
    pub app_version: String,
}

/// `None` when WebDAV is not set up. Needs the saved passwords unlocked.
pub(crate) async fn sync_target(
    app_version: &str,
    secret_state: &SecretStoreState,
) -> Result<Option<SyncTarget>, String> {
    let secret_state = secret_state.clone();
    let loaded = run_blocking(move || {
        let settings = load_settings()?;
        if !is_configured(&settings) {
            return Ok(None);
        }
        let password = saved_secret(&secret_state, PASSWORD_KEY)?;
        let backup_password = saved_secret(&secret_state, BACKUP_PASSWORD_KEY)?;
        Ok(password
            .zip(backup_password)
            .map(|passwords| (settings, passwords)))
    })
    .await?;
    let Some((settings, (password, backup_password))) = loaded else {
        return Ok(None);
    };
    let settings = normalized_settings(settings)?;
    Ok(Some(SyncTarget {
        client: WebDavClient::new(
            &settings.url,
            &settings.username,
            password,
            &user_agent(app_version),
        )?,
        directory: split_remote_directory(&settings.remote_directory)?,
        key: format!(
            "{}\n{}\n{}",
            settings.url, settings.username, settings.remote_directory
        ),
        password: backup_password,
        device: device_name(),
        app_version: app_version.to_string(),
    }))
}

fn status(secret_state: &SecretStoreState) -> Result<WebDavBackupStatus, String> {
    let saved = secret_state.saved_keys()?;
    Ok(WebDavBackupStatus {
        settings: load_settings()?,
        has_password: saved.iter().any(|key| key == PASSWORD_KEY),
        has_backup_password: saved.iter().any(|key| key == BACKUP_PASSWORD_KEY),
        device_name: device_name(),
    })
}

fn saved_secret(
    secret_state: &SecretStoreState,
    key: &str,
) -> Result<Option<Zeroizing<String>>, String> {
    let data_key = secret_state.data_key()?;
    tterm_core::db::read(|connection| get_secret(connection, &data_key, key))
}

/// The saved backup password, which opens backups this account uploaded.
pub(crate) fn saved_backup_password(
    secret_state: &SecretStoreState,
) -> Result<Option<Zeroizing<String>>, String> {
    saved_secret(secret_state, BACKUP_PASSWORD_KEY)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// The same server and account, so the saved WebDAV password applies.
fn same_account(a: &WebDavBackupSettings, b: &WebDavBackupSettings) -> bool {
    a.url == b.url && a.username == b.username
}

/// Whether restoring `settings` from a backup points uploads at another
/// server or account than the one set up now.
pub(crate) fn restore_changes_account(settings: &WebDavBackupSettings) -> Result<bool, String> {
    Ok(account_change(settings)?.is_some())
}

/// The WebDAV account a backup's settings would replace, shown before import.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavAccountChange {
    /// `None` when no server is set up now.
    pub from: Option<String>,
    pub to: String,
}

/// The account change restoring `settings` makes, if any.
pub(crate) fn account_change(
    settings: &WebDavBackupSettings,
) -> Result<Option<WebDavAccountChange>, String> {
    // Settings that cannot be restored fail the import later anyway.
    let Ok(settings) = normalized_settings(settings.clone()) else {
        return Ok(None);
    };
    let current = load_settings()?;
    if same_account(&settings, &current) {
        return Ok(None);
    }
    Ok(Some(WebDavAccountChange {
        from: is_configured(&current).then(|| account_label(&current)),
        to: account_label(&settings),
    }))
}

fn account_label(settings: &WebDavBackupSettings) -> String {
    format!("{} · {}", settings.username, settings.url)
}

fn is_configured(settings: &WebDavBackupSettings) -> bool {
    !settings.url.trim().is_empty() && !settings.username.trim().is_empty()
}

fn normalized_settings(mut settings: WebDavBackupSettings) -> Result<WebDavBackupSettings, String> {
    settings.url = tterm_core::backup::webdav::parse_server_url(&settings.url)?.to_string();
    settings.username = settings.username.trim().to_string();
    if settings.username.is_empty() {
        return Err("Enter the WebDAV user name.".to_string());
    }
    settings.remote_directory = split_remote_directory(&settings.remote_directory)?.join("/");
    if !matches!(settings.frequency.as_str(), "off" | "daily" | "weekly") {
        return Err("WebDAV backup frequency must be off, daily, or weekly.".to_string());
    }
    if !(1..=50).contains(&settings.retention_count) {
        return Err("WebDAV backup retention must be between 1 and 50.".to_string());
    }
    if !settings.selection.any() {
        return Err("Select at least one category for WebDAV backups.".to_string());
    }
    if settings.selection.secrets && !settings.selection.profiles {
        return Err("Connection profiles must be included when backing up passwords.".to_string());
    }
    Ok(settings)
}

fn backup_due(settings: &WebDavBackupSettings, now: i64) -> bool {
    let interval_ms = match settings.frequency.as_str() {
        "daily" => 24 * 60 * 60 * 1_000,
        "weekly" => 7 * 24 * 60 * 60 * 1_000,
        _ => return false,
    };
    settings
        .last_backup_at
        .is_none_or(|last| now.saturating_sub(last) >= interval_ms)
}

/// The saved settings with this device's upload history.
fn load_settings() -> Result<WebDavBackupSettings, String> {
    tterm_core::db::read(read_settings)
}

pub(crate) fn read_settings(connection: &Connection) -> Result<WebDavBackupSettings, String> {
    Ok(WebDavBackupSettings {
        last_backup_at: meta::get(connection, LAST_BACKUP_AT_KEY)?,
        last_error: meta::get(connection, LAST_ERROR_KEY)?,
        ..meta::get(connection, SETTINGS_KEY)?.unwrap_or_default()
    })
}

/// Saves settings that came from the form or a backup. Upload history is
/// not theirs to change, but an error about another server no longer applies.
fn save_keeping_history(
    connection: &Connection,
    settings: WebDavBackupSettings,
) -> Result<(), String> {
    let current = read_settings(connection)?;
    let same_target = settings.url == current.url
        && settings.username == current.username
        && settings.remote_directory == current.remote_directory;
    meta::set(
        connection,
        SETTINGS_KEY,
        &WebDavBackupSettings {
            last_backup_at: None,
            last_error: None,
            ..settings
        },
    )?;
    if !same_target {
        meta::remove(connection, LAST_ERROR_KEY)?;
    }
    Ok(())
}

/// Forgets the server, its passwords and the upload history.
fn clear_settings(connection: &Connection) -> Result<(), String> {
    tterm_core::ssh::secret_store::store::delete_secret(connection, PASSWORD_KEY)?;
    tterm_core::ssh::secret_store::store::delete_secret(connection, BACKUP_PASSWORD_KEY)?;
    for key in [SETTINGS_KEY, LAST_BACKUP_AT_KEY, LAST_ERROR_KEY] {
        meta::remove(connection, key)?;
    }
    Ok(())
}

/// Moves `webdav_backup_settings.json` from older versions into the
/// database, its upload history apart from the settings.
pub(crate) fn import_settings_file(
    database: &tterm_core::db::Database,
    config_dir: &std::path::Path,
) -> Result<(), String> {
    meta::import_file(database, &config_dir.join(SETTINGS_FILE), import_settings)
}

fn import_settings(connection: &Connection, bytes: &[u8]) -> Result<(), String> {
    let mut settings = meta::parse_file::<WebDavBackupSettings>(bytes)?;
    if let Some(at) = settings.last_backup_at.take() {
        meta::set_if_absent(connection, LAST_BACKUP_AT_KEY, &at)?;
    }
    if let Some(error) = settings.last_error.take() {
        meta::set_if_absent(connection, LAST_ERROR_KEY, &error)?;
    }
    meta::set_if_absent(connection, SETTINGS_KEY, &settings)
}

/// The settings as a backup carries them, without this device's upload
/// history. `None` when WebDAV is not set up.
pub(crate) fn settings_for_backup() -> Result<Option<WebDavBackupSettings>, String> {
    let settings = load_settings()?;
    Ok(is_configured(&settings).then_some(WebDavBackupSettings {
        last_backup_at: None,
        last_error: None,
        ..settings
    }))
}

/// Applies the settings a backup carries.
pub(crate) fn restore_settings(
    connection: &Connection,
    settings: WebDavBackupSettings,
) -> Result<(), String> {
    save_keeping_history(connection, normalized_settings(settings)?)
}

#[cfg(test)]
mod tests;
