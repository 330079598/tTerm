//! Backups kept on a WebDAV server. Each upload is an ordinary
//! `.tterm-backup` archive, always encrypted with a backup password. That
//! password and the server password are kept with the other saved passwords,
//! so scheduled uploads can include credentials and need no prompt.

use super::webdav::{split_remote_directory, DavEntry, WebDavClient};
use super::{build_archive, collect_payload, value_array_len, BackupSelection, MAX_BACKUP_SIZE};
use crate::config;
use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::{get_secret, put_secret};
use crate::ssh::SecretStoreState;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, State};
use zeroize::Zeroizing;

const SETTINGS_FILE: &str = "webdav_backup_settings.json";
const DOWNLOAD_DIRECTORY: &str = "webdav-downloads";
const PASSWORD_KEY: &str = "webdav:password";
const BACKUP_PASSWORD_KEY: &str = "webdav:backup-password";
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
    #[serde(default)]
    pub last_backup_at: Option<i64>,
    /// Why the last scheduled or manual upload failed; cleared on success.
    #[serde(default)]
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

#[tauri::command]
pub async fn get_webdav_backup_status(
    secret_state: State<'_, SecretStoreState>,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || status(&secret_state)).await
}

/// Saves the settings. A password left `None` or empty keeps the saved one.
#[tauri::command]
pub async fn save_webdav_backup_settings(
    settings: WebDavBackupSettings,
    password: Option<String>,
    backup_password: Option<String>,
    secret_state: State<'_, SecretStoreState>,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.inner().clone();
    let password = non_empty(password).map(Zeroizing::new);
    let backup_password = non_empty(backup_password).map(Zeroizing::new);
    run_blocking(move || {
        let mut settings = normalized_settings(settings)?;
        if backup_password
            .as_deref()
            .is_some_and(|value| value.chars().count() < 8)
        {
            return Err("Backup password must contain at least 8 characters.".to_string());
        }
        let saved = secret_state.saved_keys()?;
        if password.is_none() && !saved.iter().any(|key| key == PASSWORD_KEY) {
            return Err("Enter the WebDAV password.".to_string());
        }
        if backup_password.is_none() && !saved.iter().any(|key| key == BACKUP_PASSWORD_KEY) {
            return Err("Enter a backup password to encrypt WebDAV backups.".to_string());
        }
        if password.is_some() || backup_password.is_some() {
            let data_key = secret_state.data_key()?;
            crate::db::write(|transaction| {
                if let Some(password) = password.as_deref() {
                    put_secret(transaction, &data_key, PASSWORD_KEY, password)?;
                }
                if let Some(password) = backup_password.as_deref() {
                    put_secret(transaction, &data_key, BACKUP_PASSWORD_KEY, password)?;
                }
                Ok(())
            })?;
        }
        // Upload history is not the form's to change, but an error about
        // another server no longer applies.
        let current = load_settings()?;
        settings.last_backup_at = current.last_backup_at;
        let same_target = settings.url == current.url
            && settings.username == current.username
            && settings.remote_directory == current.remote_directory;
        settings.last_error = current.last_error.filter(|_| same_target);
        save_settings(&settings)?;
        status(&secret_state)
    })
    .await
}

/// Forgets the server, its password and the backup password. Remote files stay.
#[tauri::command]
pub async fn clear_webdav_backup_settings(
    secret_state: State<'_, SecretStoreState>,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        crate::db::write(|transaction| {
            crate::ssh::secret_store::delete_secret(transaction, PASSWORD_KEY)?;
            crate::ssh::secret_store::delete_secret(transaction, BACKUP_PASSWORD_KEY)?;
            Ok(())
        })?;
        let path = settings_path()?;
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("Failed to delete WebDAV backup settings: {error}"))?;
        }
        status(&secret_state)
    })
    .await
}

/// Checks the server and credentials and creates the remote folder. Uses
/// the entered password when given, else the saved one.
#[tauri::command]
pub async fn test_webdav_connection(
    app: AppHandle,
    settings: WebDavBackupSettings,
    password: Option<String>,
    secret_state: State<'_, SecretStoreState>,
) -> Result<(), String> {
    let secret_state = secret_state.inner().clone();
    let password = non_empty(password).map(Zeroizing::new);
    let (settings, password) = run_blocking(move || {
        let settings = normalized_settings(settings)?;
        let password = match password {
            Some(password) => password,
            None => saved_secret(&secret_state, PASSWORD_KEY)?
                .ok_or_else(|| "Enter the WebDAV password.".to_string())?,
        };
        Ok((settings, password))
    })
    .await?;
    let client = WebDavClient::new(
        &settings.url,
        &settings.username,
        password,
        &user_agent(&app),
    )?;
    client.check().await?;
    client
        .ensure_directory(&split_remote_directory(&settings.remote_directory)?)
        .await
}

/// Uploads a backup when one is due, or now when `force` is set. A scheduled
/// run that cannot start (not configured, passwords locked, another upload
/// running) returns `None` and tries again next time.
#[tauri::command]
pub async fn run_webdav_backup(
    app: AppHandle,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Option<WebDavUploadResult>, String> {
    struct Uploading;
    impl Drop for Uploading {
        fn drop(&mut self) {
            UPLOADING.store(false, Ordering::Release);
        }
    }

    if UPLOADING.swap(true, Ordering::AcqRel) {
        return if force {
            Err("A WebDAV backup is already being uploaded.".to_string())
        } else {
            Ok(None)
        };
    }
    let _uploading = Uploading;
    run_webdav_backup_locked(app, frontend_state, force, secret_state.inner()).await
}

async fn run_webdav_backup_locked(
    app: AppHandle,
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
    let result = upload(&app, &settings, frontend_state, secret_state).await;
    let message = result.as_ref().err().cloned();
    run_blocking(move || {
        // Reload so a save made during the upload is kept.
        let mut settings = load_settings()?;
        match message {
            Some(message) => settings.last_error = Some(message),
            None => {
                settings.last_backup_at = Some(now_ms());
                settings.last_error = None;
            }
        }
        save_settings(&settings)
    })
    .await?;
    result.map(Some)
}

async fn upload(
    app: &AppHandle,
    settings: &WebDavBackupSettings,
    frontend_state: Option<Value>,
    secret_state: &SecretStoreState,
) -> Result<WebDavUploadResult, String> {
    let settings = normalized_settings(settings.clone())?;
    let directory = split_remote_directory(&settings.remote_directory)?;
    let (password, archive, profile_count, command_count, secret_count) = {
        let app = app.clone();
        let secret_state = secret_state.clone();
        let selection = settings.selection.clone();
        run_blocking(move || {
            let password = saved_secret(&secret_state, PASSWORD_KEY)?
                .ok_or_else(|| "The WebDAV password is not saved.".to_string())?;
            let backup_password = saved_secret(&secret_state, BACKUP_PASSWORD_KEY)?
                .ok_or_else(|| "The WebDAV backup password is not saved.".to_string())?;
            let payload = collect_payload(&app, &selection, frontend_state, &secret_state)?;
            let archive = build_archive(&app, &selection, &payload, Some(&backup_password))?;
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
        &user_agent(app),
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

#[tauri::command]
pub async fn list_webdav_backups(
    app: AppHandle,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<RemoteBackupEntry>, String> {
    let (client, directory) = connect(&app, secret_state.inner()).await?;
    let entries = client.list(&directory).await?;
    Ok(remote_backups(&entries, &device_name()))
}

/// Downloads a remote backup next to the config so the import flow can
/// inspect it like a local file. Returns the local path.
#[tauri::command]
pub async fn download_webdav_backup(
    app: AppHandle,
    file_name: String,
    secret_state: State<'_, SecretStoreState>,
) -> Result<String, String> {
    validate_file_name(&file_name)?;
    let (client, directory) = connect(&app, secret_state.inner()).await?;
    let data = client.get(&directory, &file_name, MAX_BACKUP_SIZE).await?;
    run_blocking(move || {
        let downloads = config::ensure_config_dir()?.join(DOWNLOAD_DIRECTORY);
        // Only the backup being restored is kept.
        if downloads.exists() {
            fs::remove_dir_all(&downloads)
                .map_err(|error| format!("Failed to clear old WebDAV downloads: {error}"))?;
        }
        fs::create_dir_all(&downloads)
            .map_err(|error| format!("Failed to create the WebDAV download folder: {error}"))?;
        let path = downloads.join(&file_name);
        config::atomic_write_private(&path, data)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

#[tauri::command]
pub async fn delete_webdav_backup(
    app: AppHandle,
    file_name: String,
    secret_state: State<'_, SecretStoreState>,
) -> Result<(), String> {
    validate_file_name(&file_name)?;
    let (client, directory) = connect(&app, secret_state.inner()).await?;
    client.delete(&directory, &file_name).await
}

async fn connect(
    app: &AppHandle,
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
        &user_agent(app),
    )?;
    Ok((client, directory))
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
    crate::db::read(|connection| get_secret(connection, &data_key, key))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn is_configured(settings: &WebDavBackupSettings) -> bool {
    !settings.url.trim().is_empty() && !settings.username.trim().is_empty()
}

fn normalized_settings(mut settings: WebDavBackupSettings) -> Result<WebDavBackupSettings, String> {
    settings.url = super::webdav::parse_server_url(&settings.url)?.to_string();
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

fn settings_path() -> Result<PathBuf, String> {
    Ok(config::ensure_config_dir()?.join(SETTINGS_FILE))
}

fn load_settings() -> Result<WebDavBackupSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(WebDavBackupSettings::default());
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Failed to read WebDAV backup settings: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse WebDAV backup settings: {error}"))
}

fn save_settings(settings: &WebDavBackupSettings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Failed to serialize WebDAV backup settings: {error}"))?;
    config::atomic_write(&settings_path()?, bytes)
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn user_agent(app: &AppHandle) -> String {
    format!("tTerm/{}", app.package_info().version)
}

fn device_name() -> String {
    sanitize_device_name(&tauri_plugin_os::hostname())
}

/// A file-name-safe device label: `Stone's MacBook.local` -> `stone-s-macbook`.
fn sanitize_device_name(hostname: &str) -> String {
    let hostname = hostname.trim();
    let hostname = hostname
        .strip_suffix(".local")
        .or_else(|| hostname.strip_suffix(".lan"))
        .unwrap_or(hostname);
    let mut name = String::new();
    for ch in hostname.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name: String = name.trim_matches('-').chars().take(32).collect();
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "device".to_string()
    } else {
        name.to_string()
    }
}

fn backup_file_name(device: &str, at: DateTime<Utc>) -> String {
    format!(
        "{FILE_PREFIX}{device}-{}{FILE_EXTENSION}",
        at.format(TIMESTAMP_FORMAT)
    )
}

/// The device and creation time encoded in a name made by `backup_file_name`.
fn parse_backup_file_name(name: &str) -> Option<(String, i64)> {
    let stem = name
        .strip_prefix(FILE_PREFIX)?
        .strip_suffix(FILE_EXTENSION)?;
    let split = stem.len().checked_sub(TIMESTAMP_LEN + 1)?;
    let (device, timestamp) = (stem.get(..split)?, stem.get(split + 1..)?);
    if device.is_empty() || stem.as_bytes()[split] != b'-' {
        return None;
    }
    let at = NaiveDateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT).ok()?;
    Some((device.to_string(), at.and_utc().timestamp_millis()))
}

fn remote_backups(entries: &[DavEntry], device: &str) -> Vec<RemoteBackupEntry> {
    let mut backups: Vec<_> = entries
        .iter()
        .filter(|entry| !entry.is_collection && entry.name.ends_with(FILE_EXTENSION))
        .map(|entry| {
            let parsed = parse_backup_file_name(&entry.name);
            RemoteBackupEntry {
                file_name: entry.name.clone(),
                size_bytes: entry.size,
                created_at: parsed.as_ref().map(|(_, at)| *at).or(entry.modified_at),
                current_device: parsed.as_ref().is_some_and(|(name, _)| name == device),
                device: parsed.map(|(name, _)| name),
            }
        })
        .collect();
    backups.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.file_name.cmp(&left.file_name))
    });
    backups
}

/// This device's uploads beyond the newest `retention`.
fn stale_backups(entries: &[DavEntry], device: &str, retention: usize) -> Vec<String> {
    remote_backups(entries, device)
        .into_iter()
        .filter(|backup| backup.current_device)
        .skip(retention)
        .map(|backup| backup.file_name)
        .collect()
}

fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 255
        || !name.ends_with(FILE_EXTENSION)
        || name.contains(['/', '\\'])
        || name.starts_with('.')
        || name.chars().any(char::is_control)
    {
        return Err("Invalid remote backup name.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn file(name: &str) -> DavEntry {
        DavEntry {
            name: name.to_string(),
            is_collection: false,
            size: 1,
            modified_at: Some(0),
        }
    }

    /// An encrypted archive survives the upload, pruning and download path.
    /// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_archive -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_archive_upload_prune_and_restore() {
        use super::super::{
            build_archive_for_version, decode_archive, BackupPayload, MigrationSecretRecord,
        };

        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        let client = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
            "tTerm-test",
        )
        .unwrap();
        let directory =
            split_remote_directory(&format!("tterm-e2e-{}/tTerm", std::process::id())).unwrap();
        client.ensure_directory(&directory).await.unwrap();

        let selection = default_selection();
        let mut payload = BackupPayload::default();
        payload.profiles = Some(serde_json::json!([]));
        payload.secrets.push(MigrationSecretRecord {
            key: "profile-id".to_string(),
            password: "top-secret".to_string(),
        });
        let archive =
            build_archive_for_version("test", &selection, &payload, Some("backup-pass")).unwrap();

        // Three uploads from this device and one from another; keep two of ours.
        for (device, day) in [("mac", 1), ("mac", 2), ("pc", 2), ("mac", 3)] {
            let at = Utc.with_ymd_and_hms(2026, 9, day, 0, 0, 0).unwrap();
            client
                .put(&directory, &backup_file_name(device, at), archive.clone())
                .await
                .unwrap();
        }
        let entries = client.list(&directory).await.unwrap();
        for name in stale_backups(&entries, "mac", 2) {
            client.delete(&directory, &name).await.unwrap();
        }
        let listed = remote_backups(&client.list(&directory).await.unwrap(), "mac");
        let names: Vec<_> = listed
            .iter()
            .map(|entry| entry.file_name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "tterm-mac-20260903-000000.tterm-backup",
                "tterm-pc-20260902-000000.tterm-backup",
                "tterm-mac-20260902-000000.tterm-backup",
            ]
        );
        assert!(listed
            .iter()
            .all(|entry| entry.size_bytes == archive.len() as u64));

        let data = client
            .get(&directory, &listed[0].file_name, MAX_BACKUP_SIZE)
            .await
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("tterm-e2e-{}.tterm-backup", std::process::id()));
        fs::write(&path, data).unwrap();
        let decoded = decode_archive(&path, Some("backup-pass")).unwrap();
        assert_eq!(
            decoded.payload.as_ref().unwrap().secrets[0].password,
            "top-secret"
        );
        assert!(decode_archive(&path, Some("wrong-pass")).is_err());
        fs::remove_file(path).unwrap();

        client.delete_directory(&directory[..1]).await.unwrap();
    }

    #[test]
    fn sanitizes_device_names() {
        assert_eq!(
            sanitize_device_name("Stone's MacBook Pro.local"),
            "stone-s-macbook-pro"
        );
        assert_eq!(sanitize_device_name("DESKTOP-AB12"), "desktop-ab12");
        assert_eq!(sanitize_device_name("王的电脑"), "device");
        assert_eq!(sanitize_device_name(""), "device");
        assert!(sanitize_device_name(&"a".repeat(80)).len() <= 32);
    }

    #[test]
    fn round_trips_backup_file_names() {
        let at = Utc.with_ymd_and_hms(2026, 9, 29, 8, 30, 5).unwrap();
        let name = backup_file_name("my-mac", at);
        assert_eq!(name, "tterm-my-mac-20260929-083005.tterm-backup");
        assert_eq!(
            parse_backup_file_name(&name),
            Some(("my-mac".to_string(), at.timestamp_millis()))
        );
        assert_eq!(
            parse_backup_file_name("tterm-20260929-083005.tterm-backup"),
            None
        );
        assert_eq!(parse_backup_file_name("manual.tterm-backup"), None);
        assert_eq!(
            parse_backup_file_name("tterm-x-2026-bad.tterm-backup"),
            None
        );
    }

    #[test]
    fn lists_newest_first_and_prunes_only_this_device() {
        let entries = vec![
            file("tterm-mac-20260901-000000.tterm-backup"),
            file("tterm-mac-20260903-000000.tterm-backup"),
            file("tterm-pc-20260902-000000.tterm-backup"),
            file("tterm-mac-20260902-000000.tterm-backup"),
            file("manual.tterm-backup"),
            file("notes.txt"),
            DavEntry {
                is_collection: true,
                ..file("tterm-mac-20260904-000000.tterm-backup")
            },
        ];
        let listed = remote_backups(&entries, "mac");
        let names: Vec<_> = listed.iter().map(|b| b.file_name.as_str()).collect();
        assert_eq!(
            names,
            [
                "tterm-mac-20260903-000000.tterm-backup",
                "tterm-pc-20260902-000000.tterm-backup",
                "tterm-mac-20260902-000000.tterm-backup",
                "tterm-mac-20260901-000000.tterm-backup",
                "manual.tterm-backup",
            ]
        );
        assert!(!listed[1].current_device);
        assert_eq!(listed[4].device, None);
        assert_eq!(
            stale_backups(&entries, "mac", 1),
            [
                "tterm-mac-20260902-000000.tterm-backup",
                "tterm-mac-20260901-000000.tterm-backup",
            ]
        );
        assert!(stale_backups(&entries, "mac", 3).is_empty());
    }

    #[test]
    fn validates_remote_file_names() {
        assert!(validate_file_name("tterm-mac-20260901-000000.tterm-backup").is_ok());
        assert!(validate_file_name("../x.tterm-backup").is_err());
        assert!(validate_file_name("a/b.tterm-backup").is_err());
        assert!(validate_file_name("backup.zip").is_err());
    }

    #[test]
    fn normalizes_and_validates_settings() {
        let settings = normalized_settings(WebDavBackupSettings {
            url: " https://dav.jianguoyun.com/dav ".to_string(),
            username: " me@example.com ".to_string(),
            remote_directory: "/tTerm//backups/".to_string(),
            ..WebDavBackupSettings::default()
        })
        .unwrap();
        assert_eq!(settings.url, "https://dav.jianguoyun.com/dav");
        assert_eq!(settings.username, "me@example.com");
        assert_eq!(settings.remote_directory, "tTerm/backups");

        let valid = || WebDavBackupSettings {
            url: "https://example.com/".to_string(),
            username: "me".to_string(),
            ..WebDavBackupSettings::default()
        };
        assert!(normalized_settings(WebDavBackupSettings {
            username: " ".into(),
            ..valid()
        })
        .is_err());
        assert!(normalized_settings(WebDavBackupSettings {
            retention_count: 0,
            ..valid()
        })
        .is_err());
        assert!(normalized_settings(WebDavBackupSettings {
            frequency: "hourly".into(),
            ..valid()
        })
        .is_err());
        let mut no_profiles = valid();
        no_profiles.selection.profiles = false;
        assert!(normalized_settings(no_profiles).is_err());
    }

    #[test]
    fn schedules_by_frequency() {
        let day = 24 * 60 * 60 * 1_000;
        let mut settings = WebDavBackupSettings::default();
        assert!(backup_due(&settings, day));
        settings.last_backup_at = Some(day);
        assert!(!backup_due(&settings, 2 * day - 1));
        assert!(backup_due(&settings, 2 * day));
        settings.frequency = "weekly".to_string();
        assert!(!backup_due(&settings, 2 * day));
        settings.frequency = "off".to_string();
        settings.last_backup_at = None;
        assert!(!backup_due(&settings, day));
    }
}
