use crate::command_library::{CommandRepository, SavedCommand};
use crate::config::{self, AppConfig};
use crate::core::blocking::run_blocking;
use crate::db::meta;
use crate::profiles::SavedProfile;
use crate::session::SessionData;
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::secret_store::{
    get_secret, put_secret, restore_secrets, snapshot_secrets, DataKey, SecretRow,
};
use crate::ssh::store::KnownHostStore;
use crate::ssh::{SecretStoreState, VerificationPurpose};
use crate::tunnel::TunnelRule;
use aes_gcm::aead::{Aead, KeyInit, Payload as AeadPayload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, State};
use zeroize::Zeroize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub mod remote;
pub(crate) mod webdav;

const FORMAT_NAME: &str = "tterm-backup";
const FORMAT_VERSION: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const PAYLOAD_ENTRY: &str = "payload.json";
const ENCRYPTED_PAYLOAD_ENTRY: &str = "payload.enc";
const MAX_BACKUP_SIZE: u64 = 128 * 1024 * 1024;
const MAX_PAYLOAD_SIZE: u64 = 64 * 1024 * 1024;
const BACKUP_AAD: &[u8] = b"tterm-backup-v1";
const AUTOMATIC_SETTINGS_KEY: &str = "settings.automatic_backup";
/// Where older versions kept the automatic backup settings.
const AUTOMATIC_SETTINGS_FILE: &str = "backup_settings.json";
/// When the last automatic backup ran, kept apart from the settings.
const AUTOMATIC_LAST_BACKUP_AT_KEY: &str = "backup.automatic.last_backup_at";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupSelection {
    #[serde(default)]
    pub settings: bool,
    #[serde(default)]
    pub profiles: bool,
    #[serde(default)]
    pub session: bool,
    #[serde(default)]
    pub known_hosts: bool,
    #[serde(default)]
    pub sftp_directories: bool,
    #[serde(default)]
    pub command_library: bool,
    #[serde(default)]
    pub themes: bool,
    #[serde(default)]
    pub secrets: bool,
    #[serde(default)]
    pub logs: bool,
}

impl BackupSelection {
    fn any(&self) -> bool {
        self.settings
            || self.profiles
            || self.session
            || self.known_hosts
            || self.sftp_directories
            || self.command_library
            || self.themes
            || self.secrets
            || self.logs
    }

    fn without_secrets(&self) -> Self {
        let mut selection = self.clone();
        selection.secrets = false;
        selection
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupExportOptions {
    pub selection: BackupSelection,
    #[serde(default)]
    pub backup_password: Option<String>,
    #[serde(default)]
    pub frontend_state: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInspectInput {
    pub input_path: String,
    #[serde(default)]
    pub backup_password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupImportOptions {
    pub selection: BackupSelection,
    #[serde(default)]
    pub backup_password: Option<String>,
    #[serde(default = "default_conflict_strategy")]
    pub conflict_strategy: String,
}

fn default_conflict_strategy() -> String {
    "merge".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupCrypto {
    algorithm: String,
    kdf: String,
    salt_b64: String,
    nonce_b64: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    format: String,
    format_version: u32,
    app_version: String,
    created_at: String,
    platform: String,
    selection: BackupSelection,
    encrypted: bool,
    payload_sha256: String,
    secret_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    crypto: Option<BackupCrypto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationSecretRecord {
    key: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupLogFile {
    relative_path: String,
    data_b64: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct BackupPayload {
    config: Option<Value>,
    profiles: Option<Value>,
    profile_groups: Option<Value>,
    /// Port-forwarding rules; travel with `profiles` because they reference profile ids.
    #[serde(default)]
    tunnels: Option<Value>,
    session: Option<Value>,
    known_hosts: Option<Value>,
    sftp_directories: Option<Value>,
    commands: Option<Vec<SavedCommand>>,
    frontend_state: Option<Value>,
    /// The WebDAV server, the local backup schedule and the sync preferences
    /// travel with `config`; the WebDAV passwords are among `secrets`.
    #[serde(default)]
    webdav_backup: Option<remote::WebDavBackupSettings>,
    #[serde(default)]
    automatic_backup: Option<AutomaticBackupSettings>,
    #[serde(default)]
    sync: Option<crate::sync::SyncSettings>,
    /// Which profile groups the sidebar shows collapsed; travels with `config`.
    #[serde(default)]
    collapsed_profile_group_keys: Option<Vec<String>>,
    #[serde(default)]
    secrets: Vec<MigrationSecretRecord>,
    #[serde(default)]
    logs: Vec<BackupLogFile>,
}

impl Drop for BackupPayload {
    fn drop(&mut self) {
        for secret in &mut self.secrets {
            secret.password.zeroize();
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInspectResult {
    pub manifest: BackupManifest,
    pub requires_password: bool,
    pub password_verified: bool,
    pub profile_count: usize,
    pub command_count: usize,
    pub secret_count: usize,
    pub has_frontend_state: bool,
    pub log_file_count: usize,
    pub diff: BackupDiff,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDiff {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDiff {
    pub profiles: CategoryDiff,
    pub commands: CategoryDiff,
    pub settings_changed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupExportResult {
    pub output_path: String,
    pub profile_count: usize,
    pub command_count: usize,
    pub secret_count: usize,
    pub encrypted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupImportResult {
    pub profiles_imported: usize,
    pub commands_imported: usize,
    pub secrets_imported: usize,
    pub frontend_state: Option<Value>,
    pub pre_import_backup_path: String,
    pub requires_restart: bool,
    pub missing_key_files: Vec<MissingKeyFile>,
}

/// A private key file imported profiles log in with that is not on this
/// device: a backup carries the path, not the file.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingKeyFile {
    pub path: String,
    /// Names of the profiles that use it.
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticBackupSettings {
    pub frequency: String,
    pub directory: String,
    pub retention_count: u16,
    pub selection: BackupSelection,
    /// From the database. Never saved with the settings, and a value sent
    /// back by the UI is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_backup_at: Option<i64>,
}

impl Default for AutomaticBackupSettings {
    fn default() -> Self {
        Self {
            frequency: "off".to_string(),
            directory: String::new(),
            retention_count: 10,
            selection: BackupSelection {
                settings: true,
                profiles: true,
                session: true,
                known_hosts: true,
                sftp_directories: true,
                command_library: true,
                themes: true,
                secrets: false,
                logs: false,
            },
            last_backup_at: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupHistoryEntry {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub modified_at: i64,
    pub kind: String,
}

struct DecodedBundle {
    manifest: BackupManifest,
    payload: Option<BackupPayload>,
}

#[tauri::command]
pub async fn export_backup(
    app: AppHandle,
    output_path: String,
    options: BackupExportOptions,
    secret_state: State<'_, SecretStoreState>,
) -> Result<BackupExportResult, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || export_backup_blocking(app, output_path, options, &secret_state)).await
}

#[tauri::command]
pub async fn inspect_backup(input: BackupInspectInput) -> Result<BackupInspectResult, String> {
    run_blocking(move || inspect_backup_blocking(input)).await
}

#[tauri::command]
pub async fn import_backup(
    app: AppHandle,
    input_path: String,
    options: BackupImportOptions,
    secret_state: State<'_, SecretStoreState>,
) -> Result<BackupImportResult, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || import_backup_blocking(app, input_path, options, &secret_state)).await
}

#[tauri::command]
pub async fn run_due_automatic_backup(
    app: AppHandle,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Option<BackupExportResult>, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        run_due_automatic_backup_blocking(app, frontend_state, force, &secret_state)
    })
    .await
}

fn export_backup_blocking(
    app: AppHandle,
    output_path: String,
    mut options: BackupExportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupExportResult, String> {
    validate_export_options(&options)?;
    // The file is readable by whoever chose its password.
    if options.selection.secrets {
        secret_state.require_verification(VerificationPurpose::Sensitive)?;
    }
    let output = normalized_backup_path(&output_path)?;
    let mut payload = collect_payload(
        &app,
        &options.selection,
        options.frontend_state.take(),
        secret_state,
    )?;
    let profile_count = value_array_len(payload.profiles.as_ref());
    let command_count = payload.commands.as_ref().map_or(0, Vec::len);
    let secret_count = payload.secrets.len();
    let encrypted = options
        .backup_password
        .as_deref()
        .is_some_and(|p| !p.is_empty());
    let archive = build_archive(
        &app,
        &options.selection,
        &payload,
        options.backup_password.as_deref(),
    )?;
    config::atomic_write_private(&output, archive)?;
    if let Some(password) = options.backup_password.as_mut() {
        password.zeroize();
    }
    payload
        .secrets
        .iter_mut()
        .for_each(|record| record.password.zeroize());
    Ok(BackupExportResult {
        output_path: output.to_string_lossy().into_owned(),
        profile_count,
        command_count,
        secret_count,
        encrypted,
    })
}

fn inspect_backup_blocking(input: BackupInspectInput) -> Result<BackupInspectResult, String> {
    let bundle = decode_archive(
        Path::new(&input.input_path),
        input.backup_password.as_deref(),
    )?;
    let payload = bundle.payload.as_ref();
    let diff = payload.map(calculate_diff).transpose()?.unwrap_or_default();
    Ok(BackupInspectResult {
        requires_password: bundle.manifest.encrypted,
        password_verified: payload.is_some(),
        profile_count: payload
            .and_then(|p| p.profiles.as_ref())
            .map_or(0, value_array_len_one),
        command_count: payload
            .and_then(|p| p.commands.as_ref())
            .map_or(0, Vec::len),
        secret_count: payload.map_or(bundle.manifest.secret_count, |p| p.secrets.len()),
        has_frontend_state: payload.and_then(|p| p.frontend_state.as_ref()).is_some(),
        log_file_count: payload.map_or(0, |p| p.logs.len()),
        diff,
        manifest: bundle.manifest,
    })
}

fn import_backup_blocking(
    app: AppHandle,
    input_path: String,
    mut options: BackupImportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupImportResult, String> {
    if !options.selection.any() {
        return Err("Select at least one data category to import.".to_string());
    }
    if !matches!(options.conflict_strategy.as_str(), "merge" | "replace") {
        return Err("Conflict strategy must be 'merge' or 'replace'.".to_string());
    }
    let mut bundle = decode_archive(Path::new(&input_path), options.backup_password.as_deref())?;
    let payload = bundle
        .payload
        .as_mut()
        .ok_or_else(|| "This backup is encrypted. Enter its backup password.".to_string())?;
    ensure_selection_available(&options.selection, &bundle.manifest.selection)?;
    validate_payload(payload)?;
    if import_redirects_webdav(payload, &options.selection)? {
        secret_state.require_verification(VerificationPurpose::Sensitive)?;
    }

    // Passwords are encrypted with the data key inside the same transaction
    // as the profiles they belong to, so the store must be unlocked first.
    let data_key = if options.selection.secrets {
        Some(secret_state.data_key()?)
    } else {
        None
    };

    let pre_import_backup_path = create_pre_import_backup(&app, &options.selection, secret_state)?;
    let file_snapshot = capture_file_snapshot(&options.selection, payload)?;
    let database_snapshot = capture_database_snapshot(&options.selection)?;
    let command_snapshot = if options.selection.command_library {
        Some(CommandRepository::new(crate::db::get()?).list()?)
    } else {
        None
    };

    let (profiles_imported, commands_imported, secrets_imported) =
        match apply_payload(payload, &options, data_key.as_ref()) {
            Ok(counts) => counts,
            Err(error) => {
                let mut rollback_errors = Vec::new();
                if let Err(err) = restore_file_snapshot(&file_snapshot) {
                    rollback_errors.push(err);
                }
                if let Err(err) = restore_database_snapshot(&database_snapshot) {
                    rollback_errors.push(err);
                }
                if let Some(commands) = command_snapshot.as_ref() {
                    if let Err(err) = replace_commands(commands) {
                        rollback_errors.push(err);
                    }
                }
                if rollback_errors.is_empty() {
                    return Err(format!("Import failed and was rolled back: {error}"));
                }
                return Err(format!(
                    "Import failed: {error}. Rollback also reported: {}",
                    rollback_errors.join("; ")
                ));
            }
        };

    if let Some(password) = options.backup_password.as_mut() {
        password.zeroize();
    }
    let imported_secrets = data_key.is_some();
    let missing_key_files = payload
        .profiles
        .as_ref()
        .filter(|_| options.selection.profiles)
        .and_then(|profiles| from_backup_value::<Vec<SavedProfile>>(profiles.clone()).ok())
        .map(|profiles| missing_key_files(&profiles))
        .unwrap_or_default();
    Ok(BackupImportResult {
        missing_key_files,
        profiles_imported,
        commands_imported,
        secrets_imported,
        frontend_state: if options.selection.themes || options.selection.settings {
            payload.frontend_state.take()
        } else {
            None
        },
        pre_import_backup_path: pre_import_backup_path.to_string_lossy().into_owned(),
        requires_restart: options.selection.settings
            || options.selection.profiles
            || options.selection.session
            || options.selection.themes
            || imported_secrets,
    })
}

#[tauri::command]
pub async fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    run_blocking(load_automatic_backup_settings).await
}

#[tauri::command]
pub async fn save_automatic_backup_settings(
    settings: AutomaticBackupSettings,
) -> Result<AutomaticBackupSettings, String> {
    run_blocking(move || {
        validate_automatic_backup_settings(&settings)?;
        crate::db::write(|transaction| write_automatic_backup_settings(transaction, &settings))?;
        load_automatic_backup_settings()
    })
    .await
}

fn run_due_automatic_backup_blocking(
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

#[tauri::command]
pub fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
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
    entries.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
    Ok(entries)
}

#[tauri::command]
pub fn delete_backup_history_entry(path: String) -> Result<bool, String> {
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

fn validate_export_options(options: &BackupExportOptions) -> Result<(), String> {
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

fn collect_payload(
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

/// Whether importing hands the WebDAV settings to another server or backup
/// password, which scheduled uploads and sync would then send saved
/// passwords to.
fn import_redirects_webdav(
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

/// Whether a backup carries the saved password `key`. The WebDAV passwords
/// go with the WebDAV settings; other secrets of a device's own setup stay.
fn secret_travels(key: &str, with_webdav: bool) -> bool {
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

fn filter_frontend_state(selection: &BackupSelection, state: Option<Value>) -> Option<Value> {
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

fn strip_sensitive_fields(value: Value) -> Value {
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

fn build_archive(
    app: &AppHandle,
    selection: &BackupSelection,
    payload: &BackupPayload,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    build_archive_for_version(
        &app.package_info().version.to_string(),
        selection,
        payload,
        password,
    )
}

fn build_archive_for_version(
    app_version: &str,
    selection: &BackupSelection,
    payload: &BackupPayload,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut payload_bytes = serde_json::to_vec(payload)
        .map_err(|error| format!("Failed to serialize backup payload: {error}"))?;
    let (stored_payload, crypto) = if let Some(password) = password.filter(|p| !p.is_empty()) {
        encrypt_payload(&payload_bytes, password).map(|(bytes, crypto)| (bytes, Some(crypto)))?
    } else {
        (payload_bytes.clone(), None)
    };
    let manifest = BackupManifest {
        format: FORMAT_NAME.to_string(),
        format_version: FORMAT_VERSION,
        app_version: app_version.to_string(),
        created_at: Utc::now().to_rfc3339(),
        platform: std::env::consts::OS.to_string(),
        selection: selection.clone(),
        encrypted: crypto.is_some(),
        payload_sha256: sha256_hex(&stored_payload),
        secret_count: payload.secrets.len(),
        crypto,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Failed to serialize backup manifest: {error}"))?;
    let cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(MANIFEST_ENTRY, options)
        .map_err(|error| format!("Failed to create backup manifest entry: {error}"))?;
    zip.write_all(&manifest_bytes)
        .map_err(|error| format!("Failed to write backup manifest: {error}"))?;
    let entry_name = if manifest.encrypted {
        ENCRYPTED_PAYLOAD_ENTRY
    } else {
        PAYLOAD_ENTRY
    };
    zip.start_file(entry_name, options)
        .map_err(|error| format!("Failed to create backup payload entry: {error}"))?;
    zip.write_all(&stored_payload)
        .map_err(|error| format!("Failed to write backup payload: {error}"))?;
    let archive = zip
        .finish()
        .map_err(|error| format!("Failed to finish backup archive: {error}"))?
        .into_inner();
    payload_bytes.zeroize();
    Ok(archive)
}

fn decode_archive(path: &Path, password: Option<&str>) -> Result<DecodedBundle, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect backup '{}': {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_BACKUP_SIZE {
        return Err("Backup file is invalid or exceeds the 128 MiB limit.".to_string());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read backup '{}': {error}", path.display()))?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Invalid tTerm backup archive: {error}"))?;
    if archive.len() != 2 {
        return Err("Invalid backup archive layout.".to_string());
    }
    let manifest: BackupManifest = {
        let mut entry = archive
            .by_name(MANIFEST_ENTRY)
            .map_err(|_| "Backup manifest is missing.".to_string())?;
        if entry.size() > 1024 * 1024 {
            return Err("Backup manifest is too large.".to_string());
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| format!("Failed to read backup manifest: {error}"))?;
        serde_json::from_slice(&data)
            .map_err(|error| format!("Invalid backup manifest: {error}"))?
    };
    validate_manifest(&manifest)?;
    let entry_name = if manifest.encrypted {
        ENCRYPTED_PAYLOAD_ENTRY
    } else {
        PAYLOAD_ENTRY
    };
    let mut stored = {
        let mut entry = archive
            .by_name(entry_name)
            .map_err(|_| "Backup payload is missing.".to_string())?;
        if entry.size() > MAX_PAYLOAD_SIZE {
            return Err("Backup payload exceeds the 64 MiB limit.".to_string());
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| format!("Failed to read backup payload: {error}"))?;
        data
    };
    if sha256_hex(&stored) != manifest.payload_sha256 {
        return Err("Backup integrity check failed.".to_string());
    }
    if manifest.encrypted && password.filter(|p| !p.is_empty()).is_none() {
        return Ok(DecodedBundle {
            manifest,
            payload: None,
        });
    }
    let mut plain = if manifest.encrypted {
        decrypt_payload(
            &stored,
            password.unwrap_or_default(),
            manifest
                .crypto
                .as_ref()
                .ok_or_else(|| "Encrypted backup has no crypto metadata.".to_string())?,
        )?
    } else {
        stored.clone()
    };
    let payload = serde_json::from_slice::<BackupPayload>(&plain)
        .map_err(|error| format!("Invalid backup payload: {error}"))?;
    plain.zeroize();
    stored.zeroize();
    validate_payload(&payload)?;
    Ok(DecodedBundle {
        manifest,
        payload: Some(payload),
    })
}

fn validate_manifest(manifest: &BackupManifest) -> Result<(), String> {
    if manifest.format != FORMAT_NAME {
        return Err("The selected file is not a tTerm backup.".to_string());
    }
    if manifest.format_version == 0 || manifest.format_version > FORMAT_VERSION {
        return Err(format!(
            "Backup format version {} is not supported by this version of tTerm.",
            manifest.format_version
        ));
    }
    if manifest.encrypted != manifest.crypto.is_some() {
        return Err("Backup encryption metadata is inconsistent.".to_string());
    }
    Ok(())
}

fn validate_payload(payload: &BackupPayload) -> Result<(), String> {
    if let Some(value) = payload.config.clone() {
        serde_json::from_value::<AppConfig>(value)
            .map_err(|error| format!("Invalid settings data in backup: {error}"))?;
    }
    if let Some(value) = payload.profiles.clone() {
        serde_json::from_value::<Vec<SavedProfile>>(value)
            .map_err(|error| format!("Invalid profile data in backup: {error}"))?;
    }
    if let Some(value) = payload.tunnels.clone() {
        serde_json::from_value::<Vec<TunnelRule>>(value)
            .map_err(|error| format!("Invalid tunnel data in backup: {error}"))?;
    }
    if let Some(value) = payload.session.clone() {
        serde_json::from_value::<SessionData>(value)
            .map_err(|error| format!("Invalid session data in backup: {error}"))?;
    }
    if let Some(value) = payload.known_hosts.clone() {
        serde_json::from_value::<KnownHostStore>(value)
            .map_err(|error| format!("Invalid known-host data in backup: {error}"))?;
    }
    if let Some(value) = payload.sftp_directories.clone() {
        serde_json::from_value::<SftpDirectoryStore>(value)
            .map_err(|error| format!("Invalid SFTP data in backup: {error}"))?;
    }
    let mut secret_keys = HashSet::new();
    for secret in &payload.secrets {
        if secret.key.trim().is_empty() || !secret_keys.insert(secret.key.as_str()) {
            return Err("Backup contains an invalid or duplicate password key.".to_string());
        }
    }
    let mut total_log_bytes = 0usize;
    let mut log_paths = HashSet::new();
    if payload.logs.len() > 5_000 {
        return Err("Backup contains too many terminal log files.".to_string());
    }
    for log in &payload.logs {
        validate_relative_path(&log.relative_path)?;
        if !log_paths.insert(log.relative_path.as_str()) {
            return Err("Backup contains a duplicate terminal log path.".to_string());
        }
        let decoded_len = BASE64
            .decode(&log.data_b64)
            .map_err(|_| "Backup contains invalid terminal log data.".to_string())?
            .len();
        total_log_bytes = total_log_bytes.saturating_add(decoded_len);
        if total_log_bytes > 48 * 1024 * 1024 {
            return Err("Terminal logs in backup exceed the 48 MiB limit.".to_string());
        }
    }
    Ok(())
}

fn encrypt_payload(plaintext: &[u8], password: &str) -> Result<(Vec<u8>, BackupCrypto), String> {
    let memory_kib = 65_536;
    let iterations = 3;
    let parallelism = 1;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce);
    let mut key_bytes = derive_key(password, &salt, memory_kib, iterations, parallelism)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("backup keys are 32 bytes");
    let ciphertext = cipher
        .encrypt(
            &Nonce::from(nonce),
            AeadPayload {
                msg: plaintext,
                aad: BACKUP_AAD,
            },
        )
        .map_err(|_| "Failed to encrypt backup payload.".to_string())?;
    key_bytes.zeroize();
    Ok((
        ciphertext,
        BackupCrypto {
            algorithm: "AES-256-GCM".to_string(),
            kdf: "Argon2id".to_string(),
            salt_b64: BASE64.encode(salt),
            nonce_b64: BASE64.encode(nonce),
            memory_kib,
            iterations,
            parallelism,
        },
    ))
}

fn decrypt_payload(
    ciphertext: &[u8],
    password: &str,
    crypto: &BackupCrypto,
) -> Result<Vec<u8>, String> {
    if crypto.algorithm != "AES-256-GCM" || crypto.kdf != "Argon2id" {
        return Err("Backup encryption algorithm is not supported.".to_string());
    }
    let salt = BASE64
        .decode(&crypto.salt_b64)
        .map_err(|_| "Backup salt is invalid.".to_string())?;
    let nonce = BASE64
        .decode(&crypto.nonce_b64)
        .map_err(|_| "Backup nonce is invalid.".to_string())?;
    let nonce = Nonce::try_from(nonce.as_slice())
        .ok()
        .filter(|_| salt.len() == 16)
        .ok_or_else(|| "Backup encryption metadata is invalid.".to_string())?;
    let mut key_bytes = derive_key(
        password,
        &salt,
        crypto.memory_kib,
        crypto.iterations,
        crypto.parallelism,
    )?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("backup keys are 32 bytes");
    let result = cipher
        .decrypt(
            &nonce,
            AeadPayload {
                msg: ciphertext,
                aad: BACKUP_AAD,
            },
        )
        .map_err(|_| "Unable to decrypt backup. Check the backup password.".to_string());
    key_bytes.zeroize();
    result
}

pub(crate) fn derive_key(
    password: &str,
    salt: &[u8],
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<[u8; 32], String> {
    if memory_kib > 262_144 || iterations > 10 || parallelism > 8 {
        return Err("Backup KDF parameters exceed supported limits.".to_string());
    }
    let params = Params::new(memory_kib, iterations, parallelism, Some(32))
        .map_err(|error| format!("Invalid backup KDF parameters: {error}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| format!("Failed to derive backup key: {error}"))?;
    Ok(key)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn ensure_selection_available(
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

fn missing_key_files(profiles: &[SavedProfile]) -> Vec<MissingKeyFile> {
    let mut missing = BTreeMap::<&str, Vec<String>>::new();
    for profile in profiles.iter().filter(|p| p.connection_type == "ssh") {
        let own = (profile.auth_method.as_deref() == Some("key"))
            .then_some(profile.private_key_path.as_deref())
            .flatten();
        let jumps = profile
            .jump_hosts
            .iter()
            .filter(|jump| profile.uses_jump_host() && jump.auth_method == "key")
            .filter_map(|jump| jump.private_key_path.as_deref());
        for path in own.into_iter().chain(jumps).map(str::trim) {
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
fn is_default_or_existing_directory(value: &str) -> bool {
    let directory = Path::new(value.trim());
    directory.as_os_str().is_empty() || (directory.is_absolute() && directory.is_dir())
}

/// Keeps what belongs to this device in imported settings: how passwords are
/// unlocked, and folders and a custom shell that do not exist here.
fn keep_device_settings(imported: &mut AppConfig, current: AppConfig) {
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

fn apply_payload(
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
fn apply_database_payload(
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
                    to_backup_value(&crate::profiles::list_saved_profiles(transaction)?)?;
                from_backup_value(merge_arrays_by_keys(existing, safe_incoming, &["id"])?)?
            } else {
                incoming_profiles
            };
            crate::profiles::replace_profiles(transaction, &profiles)?;
        }
        if let Some(incoming) = payload.profile_groups.as_ref() {
            let groups = if merge {
                let existing =
                    to_backup_value(&crate::profiles::configured_profile_groups(transaction)?)?;
                merge_string_arrays(existing, incoming.clone())?
            } else {
                incoming.clone()
            };
            crate::profiles::replace_profile_groups(
                transaction,
                &from_backup_value::<Vec<String>>(groups)?,
            )?;
        }
        // Backups made before tunnels existed carry none; keep the current rules then.
        if let Some(incoming) = payload.tunnels.as_ref() {
            let tunnels = if merge {
                let existing = to_backup_value(&crate::tunnel::list_tunnel_rules(transaction)?)?;
                merge_arrays_by_keys(existing, incoming.clone(), &["id"])?
            } else {
                incoming.clone()
            };
            crate::tunnel::replace_tunnels(
                transaction,
                &from_backup_value::<Vec<TunnelRule>>(tunnels)?,
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

fn to_backup_value<T: Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| format!("Failed to serialize backup data: {error}"))
}

fn from_backup_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("Invalid backup data: {error}"))
}

/// Database-held data a failed import restores.
#[derive(Default)]
struct DatabaseSnapshot {
    profiles: Option<(Vec<SavedProfile>, Vec<String>, Vec<TunnelRule>)>,
    /// Ciphertext as stored. Taken with profiles too: replacing profiles
    /// deletes the passwords of the removed ones.
    secrets: Option<Vec<SecretRow>>,
    known_hosts: Option<KnownHostStore>,
    sftp_directories: Option<SftpDirectoryStore>,
    themes: Option<Vec<Value>>,
    /// The `app_meta` values an import may change: settings and the session.
    meta: Vec<(&'static str, Option<String>)>,
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

fn capture_database_snapshot(selection: &BackupSelection) -> Result<DatabaseSnapshot, String> {
    crate::db::read(|connection| snapshot_database(connection, selection))
}

fn snapshot_database(
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

fn restore_database_snapshot(snapshot: &DatabaseSnapshot) -> Result<(), String> {
    crate::db::write(|transaction| restore_database(transaction, snapshot))
}

fn restore_database(
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

fn replace_commands(commands: &[SavedCommand]) -> Result<(), String> {
    let repository = CommandRepository::new(crate::db::get()?);
    for existing in repository.list()? {
        repository.delete(&existing.id)?;
    }
    for command in commands {
        repository.save(command)?;
    }
    Ok(())
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Null);
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Failed to read '{}': {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse '{}': {error}", path.display()))
}

fn merge_arrays_by_keys(existing: Value, incoming: Value, keys: &[&str]) -> Result<Value, String> {
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

fn json_key(value: &Value, keys: &[&str]) -> Result<String, String> {
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

fn calculate_diff(payload: &BackupPayload) -> Result<BackupDiff, String> {
    let existing_profiles = to_backup_value(&crate::profiles::load_profiles()?)?;
    let profiles = calculate_json_array_diff(
        existing_profiles
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        payload
            .profiles
            .as_ref()
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        &["id"],
    )?;

    let existing_commands = CommandRepository::new(crate::db::get()?).list()?;
    let existing_by_id = existing_commands
        .into_iter()
        .map(|command| (command.id.clone(), command))
        .collect::<HashMap<_, _>>();
    let mut commands = CategoryDiff::default();
    for command in payload.commands.as_deref().unwrap_or(&[]) {
        match existing_by_id.get(&command.id) {
            None => commands.added += 1,
            Some(existing) if existing == command => commands.unchanged += 1,
            Some(_) => commands.updated += 1,
        }
    }

    let settings_changed = match payload.config.as_ref() {
        Some(imported) => {
            read_json_value(&config::get_config_path()?.join("config.json"))? != *imported
        }
        None => false,
    };
    Ok(BackupDiff {
        profiles,
        commands,
        settings_changed,
    })
}

fn calculate_json_array_diff(
    existing: &[Value],
    incoming: &[Value],
    keys: &[&str],
) -> Result<CategoryDiff, String> {
    let mut existing_by_key = HashMap::new();
    for value in existing {
        existing_by_key.insert(json_key(value, keys)?, value);
    }
    let mut diff = CategoryDiff::default();
    for value in incoming {
        match existing_by_key.get(&json_key(value, keys)?) {
            None => diff.added += 1,
            Some(existing) if **existing == *value => diff.unchanged += 1,
            Some(_) => diff.updated += 1,
        }
    }
    Ok(diff)
}

fn configured_log_root() -> Result<PathBuf, String> {
    let config_value = config::load_config_file()?;
    if config_value.terminal_log_directory.trim().is_empty() {
        Ok(config::get_config_path()?.join("logs"))
    } else {
        Ok(PathBuf::from(config_value.terminal_log_directory.trim()))
    }
}

fn imported_logs_root() -> Result<PathBuf, String> {
    Ok(config::get_config_path()?.join("logs").join("imported"))
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

fn validate_relative_path(value: &str) -> Result<(), String> {
    if value.is_empty() || value.contains('\\') {
        return Err("Backup contains an invalid relative path.".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Backup contains an unsafe relative path.".to_string());
    }
    Ok(())
}

fn restore_log_files(logs: &[BackupLogFile]) -> Result<(), String> {
    let root = imported_logs_root()?;
    for log in logs {
        validate_relative_path(&log.relative_path)?;
        let bytes = BASE64
            .decode(&log.data_b64)
            .map_err(|_| "Backup contains invalid terminal log data.".to_string())?;
        config::atomic_write_private(&root.join(Path::new(&log.relative_path)), bytes)?;
    }
    Ok(())
}

fn capture_file_snapshot(
    selection: &BackupSelection,
    payload: &BackupPayload,
) -> Result<Vec<(PathBuf, Option<Vec<u8>>)>, String> {
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

fn restore_file_snapshot(snapshot: &[(PathBuf, Option<Vec<u8>>)]) -> Result<(), String> {
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

fn create_pre_import_backup(
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
fn restore_automatic_backup_settings(
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
fn adopt_automatic_backup_settings(
    mut imported: AutomaticBackupSettings,
    current: AutomaticBackupSettings,
) -> AutomaticBackupSettings {
    if !is_default_or_existing_directory(&imported.directory) {
        imported.directory = current.directory;
    }
    imported
}

/// The saved schedule with when the last automatic backup ran.
fn load_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
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

fn write_automatic_backup_settings(
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

fn validate_automatic_backup_settings(settings: &AutomaticBackupSettings) -> Result<(), String> {
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
    backups.sort_by(|left, right| right.0.cmp(&left.0));
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

fn normalized_backup_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Backup output path is required.".to_string());
    }
    let mut path = PathBuf::from(trimmed);
    if path.extension().and_then(|value| value.to_str()) != Some("tterm-backup") {
        path.set_extension("tterm-backup");
    }
    if path.file_name().is_none() {
        return Err("Backup output path is invalid.".to_string());
    }
    Ok(path)
}

fn value_array_len(value: Option<&Value>) -> usize {
    value.map_or(0, value_array_len_one)
}

fn value_array_len_one(value: &Value) -> usize {
    value.as_array().map_or(0, Vec::len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_profile(id: &str, name: &str, group: &str) -> Value {
        serde_json::json!({
            "id": id, "name": name, "group": group, "connection_type": "ssh",
            "host": format!("{id}.example"), "port": 22, "username": "root",
            "auth_method": "password", "private_key_path": null
        })
    }

    fn db_known_host(host: &str, fingerprint: &str) -> Value {
        serde_json::json!({
            "profile_id": null, "profile_name": host, "host": host, "port": 22,
            "algorithm": "ssh-ed25519", "fingerprint": fingerprint, "trusted_at": 1
        })
    }

    fn database_selection() -> BackupSelection {
        BackupSelection {
            profiles: true,
            known_hosts: true,
            sftp_directories: true,
            ..BackupSelection::default()
        }
    }

    fn import_options(strategy: &str) -> BackupImportOptions {
        BackupImportOptions {
            selection: database_selection(),
            backup_password: None,
            conflict_strategy: strategy.to_string(),
        }
    }

    /// Seeds the database with profiles a and b, group "ops", tunnel t1, a
    /// known host and an SFTP directory.
    fn seed_database(connection: &rusqlite::Connection) {
        let payload = {
            let mut payload = BackupPayload::default();
            payload.profiles = Some(serde_json::json!([
                db_profile("a", "Alpha", "ops"),
                db_profile("b", "Beta", "")
            ]));
            payload.profile_groups = Some(serde_json::json!(["ops"]));
            payload.tunnels = Some(serde_json::json!([{
                "id": "t1", "name": "db", "profileId": "a", "kind": "local",
                "bindHost": "127.0.0.1", "bindPort": 5432,
                "destHost": "db", "destPort": 5432
            }]));
            payload.known_hosts =
                Some(serde_json::json!({ "entries": [db_known_host("a.example", "old")] }));
            payload.sftp_directories = Some(serde_json::json!({ "entries": [
                { "host": "a.example", "port": 22, "username": "root", "last_path": "/a" }
            ]}));
            payload
        };
        apply_database_payload(connection, &payload, &import_options("replace")).expect("seed");
    }

    fn incoming_payload() -> BackupPayload {
        let mut payload = BackupPayload::default();
        payload.profiles = Some(serde_json::json!([
            db_profile("b", "Beta renamed", "dev"),
            db_profile("c", "Gamma", "dev")
        ]));
        payload.profile_groups = Some(serde_json::json!(["dev"]));
        payload.known_hosts = Some(serde_json::json!({ "entries": [
            db_known_host("a.example", "old"),
            db_known_host("c.example", "new")
        ]}));
        payload.sftp_directories = Some(serde_json::json!({ "entries": [
            { "host": "a.example", "port": 22, "username": "root", "last_path": "/b" }
        ]}));
        payload
    }

    fn profile_summary(connection: &rusqlite::Connection) -> Vec<(String, String, String)> {
        crate::profiles::list_saved_profiles(connection)
            .unwrap()
            .into_iter()
            .map(|p| (p.id, p.name, p.group))
            .collect()
    }

    fn owned(rows: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
        rows.iter()
            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
            .collect()
    }

    fn webdav_settings(url: &str) -> remote::WebDavBackupSettings {
        remote::WebDavBackupSettings {
            url: url.to_string(),
            username: "me".to_string(),
            ..remote::WebDavBackupSettings::default()
        }
    }

    fn settings_and_session_selection() -> BackupSelection {
        BackupSelection {
            settings: true,
            session: true,
            themes: true,
            ..BackupSelection::default()
        }
    }

    /// Settings, upload history, a session and a theme as this device has them.
    fn seed_settings(connection: &rusqlite::Connection) -> Result<(), String> {
        write_automatic_backup_settings(
            connection,
            &AutomaticBackupSettings {
                frequency: "weekly".to_string(),
                ..AutomaticBackupSettings::default()
            },
        )?;
        meta::set(connection, AUTOMATIC_LAST_BACKUP_AT_KEY, &7_i64)?;
        remote::restore_settings(connection, webdav_settings("https://a.example/dav"))?;
        meta::set(connection, "backup.webdav.last_backup_at", &8_i64)?;
        meta::set(connection, remote::LAST_ERROR_KEY, &"timeout")?;
        crate::sync::restore_settings(
            connection,
            &crate::sync::SyncSettings {
                enabled: true,
                ..crate::sync::SyncSettings::default()
            },
        )?;
        crate::session::write_session(
            connection,
            crate::session::SessionData {
                active_tab_id: Some("mine".to_string()),
                ..crate::session::SessionData::default()
            },
        )?;
        crate::app_state::set_collapsed_profile_group_keys(connection, &["mine".to_string()])?;
        crate::themes::replace_themes(connection, &[serde_json::json!({"id": "mine"})])
    }

    fn settings_summary(connection: &rusqlite::Connection) -> Value {
        let automatic = read_automatic_backup_settings(connection).unwrap();
        let webdav = remote::read_settings(connection).unwrap();
        serde_json::json!({
            "automatic": [automatic.frequency, automatic.last_backup_at],
            "webdav": [webdav.url, webdav.last_backup_at, webdav.last_error],
            "sync": crate::sync::read_settings(connection).unwrap().enabled,
            "session": crate::session::read_session(connection).unwrap().and_then(|s| s.active_tab_id),
            "themes": crate::themes::list_themes(connection).unwrap(),
            "collapsed": crate::app_state::collapsed_profile_group_keys(connection).unwrap(),
        })
    }

    #[test]
    fn imported_settings_and_session_land_in_the_database_and_roll_back() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_settings(connection)?;
                let before = settings_summary(connection);
                let snapshot = snapshot_database(connection, &settings_and_session_selection())?;

                let mut payload = BackupPayload::default();
                payload.automatic_backup = Some(AutomaticBackupSettings {
                    frequency: "daily".to_string(),
                    // Another device's history never replaces this one's.
                    last_backup_at: Some(1),
                    ..AutomaticBackupSettings::default()
                });
                payload.webdav_backup = Some(remote::WebDavBackupSettings {
                    last_backup_at: Some(1),
                    last_error: Some("theirs".to_string()),
                    ..webdav_settings("https://b.example/dav")
                });
                payload.sync = Some(crate::sync::SyncSettings::default());
                payload.session = Some(serde_json::json!({
                    "tabs": [], "active_tab_id": "theirs", "last_saved": 2
                }));
                payload.frontend_state =
                    Some(serde_json::json!({"customThemes": [{"id": "theirs"}]}));
                payload.collapsed_profile_group_keys = Some(vec!["theirs".to_string()]);
                let options = BackupImportOptions {
                    selection: settings_and_session_selection(),
                    backup_password: None,
                    conflict_strategy: "replace".to_string(),
                };
                apply_database_payload(connection, &payload, &options)?;

                assert_eq!(
                    settings_summary(connection),
                    serde_json::json!({
                        "automatic": ["daily", 7],
                        // A new server: the old server's error no longer applies.
                        "webdav": ["https://b.example/dav", 8, null],
                        "sync": false,
                        "session": "theirs",
                        "themes": [{"id": "theirs"}],
                        "collapsed": ["theirs"],
                    })
                );
                // Saved settings never carry the upload history.
                let saved: Value = meta::get(connection, AUTOMATIC_SETTINGS_KEY)?.unwrap();
                assert!(saved.get("lastBackupAt").is_none());
                let saved: Value = meta::get(connection, remote::SETTINGS_KEY)?.unwrap();
                assert!(saved.get("lastBackupAt").is_none() && saved.get("lastError").is_none());

                restore_database(connection, &snapshot)?;
                assert_eq!(settings_summary(connection), before);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn unselected_settings_and_session_are_not_touched() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_settings(connection)?;
                let before = settings_summary(connection);
                let mut payload = BackupPayload::default();
                payload.sync = Some(crate::sync::SyncSettings::default());
                payload.session = Some(serde_json::json!({
                    "tabs": [], "active_tab_id": "theirs", "last_saved": 2
                }));
                payload.collapsed_profile_group_keys = Some(vec!["theirs".to_string()]);
                apply_database_payload(connection, &payload, &import_options("replace"))?;
                assert_eq!(settings_summary(connection), before);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn merged_collapsed_groups_add_to_this_devices() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                crate::app_state::set_collapsed_profile_group_keys(
                    connection,
                    &["ops".to_string(), "dev".to_string()],
                )?;
                let mut payload = BackupPayload::default();
                payload.collapsed_profile_group_keys =
                    Some(vec!["dev".to_string(), "prod".to_string()]);
                let options = BackupImportOptions {
                    selection: settings_and_session_selection(),
                    backup_password: None,
                    conflict_strategy: "merge".to_string(),
                };
                apply_database_payload(connection, &payload, &options)?;
                assert_eq!(
                    crate::app_state::collapsed_profile_group_keys(connection)?,
                    ["ops", "dev", "prod"]
                );

                // Backups made before the field existed leave the state alone.
                apply_database_payload(connection, &BackupPayload::default(), &options)?;
                assert_eq!(
                    crate::app_state::collapsed_profile_group_keys(connection)?,
                    ["ops", "dev", "prod"]
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn merge_import_keeps_existing_data_and_updates_by_key() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_database(connection);
                let imported = apply_database_payload(
                    connection,
                    &incoming_payload(),
                    &import_options("merge"),
                )?;
                assert_eq!(imported, 2);
                assert_eq!(
                    profile_summary(connection),
                    owned(&[
                        ("a", "Alpha", "ops"),
                        ("b", "Beta renamed", "dev"),
                        ("c", "Gamma", "dev")
                    ])
                );
                assert_eq!(
                    crate::profiles::configured_profile_groups(connection)?,
                    ["dev", "ops"]
                );
                // Tunnels missing from an old backup are left alone.
                assert_eq!(crate::tunnel::list_tunnel_rules(connection)?.len(), 1);
                let hosts = crate::ssh::store::list_known_hosts(connection)?.entries;
                assert_eq!(
                    hosts.len(),
                    2,
                    "the duplicate known host is not added twice"
                );
                let dirs = crate::sftp::store::list_sftp_directories(connection)?.entries;
                assert_eq!(dirs.len(), 1);
                assert_eq!(dirs[0].last_path, "/b");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn merge_import_keeps_known_hosts_of_profiles_sharing_a_host_key() {
        let entry = |profile_id: Option<&str>, name: &str| {
            serde_json::json!({
                "profile_id": profile_id, "profile_name": name,
                "host": "10.0.0.1", "port": 7006,
                "algorithm": "ssh-ed25519", "fingerprint": "SHA256:same", "trusted_at": 1
            })
        };
        let mut payload = BackupPayload::default();
        payload.known_hosts = Some(serde_json::json!({ "entries": [
            entry(Some("p-ubuntu"), "ubuntu"),
            entry(Some("p-test"), "test"),
            entry(None, "jump:10.0.0.1:7006"),
            // Written before profile ids were recorded: no `profile_id` key.
            {
                "profile_name": "legacy", "host": "10.0.0.1", "port": 7006,
                "algorithm": "ssh-ed25519", "fingerprint": "SHA256:same", "trusted_at": 1
            }
        ]}));
        let mut options = import_options("merge");
        options.selection = BackupSelection {
            known_hosts: true,
            ..BackupSelection::default()
        };
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                apply_database_payload(connection, &payload, &options)?;
                apply_database_payload(connection, &payload, &options)?;
                let names = crate::ssh::store::list_known_hosts(connection)?
                    .entries
                    .into_iter()
                    .map(|entry| entry.profile_name)
                    .collect::<Vec<_>>();
                assert_eq!(names, ["ubuntu", "test", "jump:10.0.0.1:7006", "legacy"]);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn merge_import_never_lets_a_name_only_entry_replace_a_profile_entry() {
        let mut payload = BackupPayload::default();
        payload.known_hosts = Some(serde_json::json!({ "entries": [
            {
                "profile_id": "p1", "profile_name": "K8s-master", "host": "10.0.0.31",
                "port": 22, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:current",
                "trusted_at": 2
            },
            {
                "profile_id": null, "profile_name": "K8s-master", "host": "10.0.0.31",
                "port": 22, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:stale",
                "trusted_at": 1
            }
        ]}));
        let mut options = import_options("merge");
        options.selection = BackupSelection {
            known_hosts: true,
            ..BackupSelection::default()
        };
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                apply_database_payload(connection, &payload, &options)?;
                let entries = crate::ssh::store::list_known_hosts(connection)?.entries;
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].profile_id.as_deref(), Some("p1"));
                assert_eq!(entries[0].fingerprint, "SHA256:current");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replacing_profiles_keeps_passwords_of_profiles_that_stay() {
        let database = crate::db::Database::open_in_memory().unwrap();
        let data_key = DataKey::generate();
        let profile = |id: &str| -> SavedProfile {
            serde_json::from_value(serde_json::json!({
                "id": id,
                "name": id,
                "connection_type": "ssh",
                "host": "example.com",
                "port": 22,
                "username": "root",
                "auth_method": "password",
            }))
            .unwrap()
        };
        let keys = |connection: &rusqlite::Connection| -> Vec<String> {
            snapshot_secrets(connection)
                .unwrap()
                .into_iter()
                .map(|row| row.key)
                .collect()
        };
        database
            .write(|connection| {
                crate::profiles::replace_profiles(connection, &[profile("keep"), profile("gone")])?;
                put_secret(connection, &data_key, "keep", "k")?;
                put_secret(connection, &data_key, "gone", "g")?;
                let before = snapshot_database(connection, &database_selection())?;

                crate::profiles::replace_profiles(connection, &[profile("keep"), profile("new")])?;
                assert_eq!(keys(connection), ["keep"]);

                // Undoing the import brings the removed profile's password back.
                restore_database(connection, &before)?;
                assert_eq!(keys(connection), ["gone", "keep"]);
                let restored = get_secret(connection, &data_key, "gone")?.unwrap();
                assert_eq!(restored.as_str(), "g");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replace_import_drops_data_missing_from_the_backup() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_database(connection);
                apply_database_payload(
                    connection,
                    &incoming_payload(),
                    &import_options("replace"),
                )?;
                assert_eq!(
                    profile_summary(connection),
                    owned(&[("b", "Beta renamed", "dev"), ("c", "Gamma", "dev")])
                );
                assert_eq!(
                    crate::profiles::configured_profile_groups(connection)?,
                    ["dev"]
                );
                assert_eq!(crate::tunnel::list_tunnel_rules(connection)?.len(), 1);
                assert_eq!(
                    crate::ssh::store::list_known_hosts(connection)?
                        .entries
                        .len(),
                    2
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn unselected_categories_are_not_touched() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_database(connection);
                let mut options = import_options("replace");
                options.selection = BackupSelection {
                    known_hosts: true,
                    ..BackupSelection::default()
                };
                apply_database_payload(connection, &incoming_payload(), &options)?;
                assert_eq!(profile_summary(connection).len(), 2);
                assert_eq!(
                    crate::sftp::store::list_sftp_directories(connection)?.entries[0].last_path,
                    "/a"
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn database_snapshot_undoes_an_import() {
        let database = crate::db::Database::open_in_memory().unwrap();
        database
            .write(|connection| {
                seed_database(connection);
                let before = snapshot_database(connection, &database_selection())?;
                let before_json = |s: &DatabaseSnapshot| {
                    serde_json::json!({
                        "profiles": s.profiles.as_ref().map(|(p, g, t)| serde_json::json!([p, g, t])),
                        "known_hosts": s.known_hosts,
                        "sftp": s.sftp_directories,
                    })
                };
                apply_database_payload(connection, &incoming_payload(), &import_options("replace"))?;
                restore_database(connection, &before)?;
                let after = snapshot_database(connection, &database_selection())?;
                assert_eq!(before_json(&after), before_json(&before));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn exported_data_imports_back_unchanged() {
        let source = crate::db::Database::open_in_memory().unwrap();
        source
            .write(|connection| {
                seed_database(connection);
                Ok(())
            })
            .unwrap();
        let payload = source
            .read(|connection| {
                Ok({
                    let mut payload = BackupPayload::default();
                    payload.profiles = Some(to_backup_value(
                        &crate::profiles::list_saved_profiles(connection)?,
                    )?);
                    payload.profile_groups = Some(to_backup_value(
                        &crate::profiles::configured_profile_groups(connection)?,
                    )?);
                    payload.tunnels = Some(to_backup_value(&crate::tunnel::list_tunnel_rules(
                        connection,
                    )?)?);
                    payload.known_hosts = Some(to_backup_value(
                        &crate::ssh::store::list_known_hosts(connection)?,
                    )?);
                    payload.sftp_directories = Some(to_backup_value(
                        &crate::sftp::store::list_sftp_directories(connection)?,
                    )?);
                    payload
                })
            })
            .unwrap();
        validate_payload(&payload).expect("exported payload validates");

        let target = crate::db::Database::open_in_memory().unwrap();
        target
            .write(|connection| {
                apply_database_payload(connection, &payload, &import_options("replace"))?;
                let copied = snapshot_database(connection, &database_selection())?;
                let original = source.read(|c| snapshot_database(c, &database_selection()))?;
                assert_eq!(
                    serde_json::to_value(copied.profiles).unwrap(),
                    serde_json::to_value(original.profiles).unwrap()
                );
                Ok(())
            })
            .unwrap();
    }

    /// Imports real backup files made before the database existed. Set
    /// `TTERM_BACKUP_FIXTURES` to a `:`-separated list of copies.
    #[test]
    #[ignore]
    fn real_pre_database_backups_import() {
        let paths = std::env::var("TTERM_BACKUP_FIXTURES").expect("TTERM_BACKUP_FIXTURES");
        for path in paths.split(':') {
            let bundle = decode_archive(Path::new(path), None).expect("decode backup");
            eprintln!("{path}: manifest {:?}", bundle.manifest.selection);
            let Some(payload) = bundle.payload.as_ref() else {
                eprintln!("  encrypted, skipped");
                continue;
            };
            validate_payload(payload).expect("old backup validates");
            let expected_profiles = payload
                .profiles
                .as_ref()
                .map(|value| {
                    serde_json::from_value::<Vec<SavedProfile>>(value.clone())
                        .unwrap()
                        .len()
                })
                .unwrap_or(0);
            for strategy in ["replace", "merge"] {
                let database = crate::db::Database::open_in_memory().unwrap();
                database
                    .write(|connection| {
                        let mut options = import_options(strategy);
                        options.selection = BackupSelection {
                            profiles: payload.profiles.is_some(),
                            known_hosts: payload.known_hosts.is_some(),
                            sftp_directories: payload.sftp_directories.is_some(),
                            ..BackupSelection::default()
                        };
                        // Merging twice must not duplicate anything.
                        apply_database_payload(connection, payload, &options)?;
                        apply_database_payload(connection, payload, &options)?;
                        let profiles = crate::profiles::list_saved_profiles(connection)?.len();
                        let known = crate::ssh::store::list_known_hosts(connection)?.entries.len();
                        let sftp = crate::sftp::store::list_sftp_directories(connection)?
                            .entries
                            .len();
                        eprintln!(
                            "  {strategy}: {profiles} profiles, {known} known hosts, {sftp} SFTP dirs, tunnels in backup: {}",
                            payload.tunnels.is_some()
                        );
                        assert_eq!(profiles, expected_profiles);
                        if let Some(value) = payload.known_hosts.as_ref() {
                            let expected = value["entries"].as_array().map_or(0, Vec::len);
                            assert_eq!(known, expected, "{strategy} keeps every known host");
                        }
                        Ok(())
                    })
                    .unwrap();
            }
        }
    }

    #[test]
    fn encrypted_payload_round_trip_and_rejects_wrong_password() {
        let plaintext = br#"{"profiles":[],"secrets":[{"key":"p","password":"secret"}]}"#;
        let (ciphertext, crypto) =
            encrypt_payload(plaintext, "correct horse battery staple").unwrap();
        assert_ne!(ciphertext, plaintext);
        assert_eq!(
            decrypt_payload(&ciphertext, "correct horse battery staple", &crypto).unwrap(),
            plaintext
        );
        assert!(decrypt_payload(&ciphertext, "wrong password", &crypto).is_err());
    }

    #[test]
    fn profile_merge_updates_by_id_and_preserves_other_profiles() {
        let existing = serde_json::json!([
            {"id":"1","name":"old"},
            {"id":"2","name":"keep"}
        ]);
        let incoming = serde_json::json!([
            {"id":"1","name":"new"},
            {"id":"3","name":"add"}
        ]);
        let merged = merge_arrays_by_keys(existing, incoming, &["id"]).unwrap();
        assert_eq!(merged.as_array().unwrap().len(), 3);
        assert_eq!(merged[0]["name"], "new");
    }

    #[test]
    fn backups_without_tunnels_still_load_and_validate() {
        let payload: BackupPayload =
            serde_json::from_str(r#"{"profiles":[]}"#).expect("old payload should deserialize");
        assert!(payload.tunnels.is_none());
        validate_payload(&payload).expect("old payload should validate");
    }

    #[test]
    fn tunnel_data_is_validated_on_import() {
        let mut payload = BackupPayload::default();
        payload.tunnels = Some(serde_json::json!([{
            "id": "t1", "name": "db", "profileId": "p1", "kind": "local",
            "bindHost": "127.0.0.1", "bindPort": 5432,
            "destHost": "db.internal", "destPort": 5432
        }]));
        validate_payload(&payload).expect("well-formed tunnels are accepted");

        payload.tunnels = Some(serde_json::json!([{ "id": "t1", "kind": "sideways" }]));
        assert!(validate_payload(&payload).is_err());
    }

    #[test]
    fn kdf_parameters_are_bounded() {
        assert!(derive_key("password", &[0; 16], 262_145, 3, 1).is_err());
        assert!(derive_key("password", &[0; 16], 65_536, 11, 1).is_err());
    }

    #[test]
    fn terminal_log_paths_reject_traversal_and_platform_separators() {
        assert!(validate_relative_path("session/a.log").is_ok());
        assert!(validate_relative_path("../secret.txt").is_err());
        assert!(validate_relative_path("session\\a.log").is_err());
        assert!(validate_relative_path("/absolute.log").is_err());
    }

    #[test]
    fn automatic_backups_never_accept_credentials() {
        let mut settings = AutomaticBackupSettings::default();
        settings.selection.secrets = true;
        assert!(validate_automatic_backup_settings(&settings).is_err());
    }

    #[test]
    fn imported_schedule_keeps_this_devices_folder_unless_the_backups_exists() {
        let existing = std::env::temp_dir().to_string_lossy().into_owned();
        let current = AutomaticBackupSettings {
            directory: existing.clone(),
            ..AutomaticBackupSettings::default()
        };
        let adopt = |directory: &str| {
            let imported = AutomaticBackupSettings {
                frequency: "weekly".to_string(),
                directory: directory.to_string(),
                ..AutomaticBackupSettings::default()
            };
            adopt_automatic_backup_settings(imported, current.clone())
        };
        let missing = std::env::temp_dir().join("tterm-no-such-backup-folder");
        let adopted = adopt(&missing.to_string_lossy());
        assert_eq!(adopted.directory, existing);
        assert_eq!(adopted.frequency, "weekly");
        assert_eq!(adopt("relative/folder").directory, existing);
        // The default folder is valid on every device.
        assert_eq!(adopt("").directory, "");
        assert_eq!(adopt(&existing).directory, existing);
    }

    #[test]
    fn imported_settings_keep_this_devices_unlock_mode_and_missing_paths() {
        let existing = std::env::temp_dir().to_string_lossy().into_owned();
        let missing = std::env::temp_dir()
            .join("tterm-no-such-path")
            .to_string_lossy()
            .into_owned();
        let current = AppConfig {
            secret_storage_mode: "this-device".to_string(),
            terminal_log_directory: existing.clone(),
            zmodem_download_directory: existing.clone(),
            terminal_shell: "pwsh".to_string(),
            ..AppConfig::default()
        };
        let mut imported = AppConfig {
            secret_storage_mode: "other-device".to_string(),
            terminal_log_directory: missing.clone(),
            zmodem_download_directory: String::new(),
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: missing,
            terminal_shell_custom_args: "--login".to_string(),
            font_size: 21,
            ..AppConfig::default()
        };
        keep_device_settings(&mut imported, current.clone());
        assert_eq!(imported.secret_storage_mode, "this-device");
        assert_eq!(imported.terminal_log_directory, existing);
        assert_eq!(imported.zmodem_download_directory, "");
        assert_eq!(imported.terminal_shell, "pwsh");
        assert_eq!(imported.terminal_shell_custom_path, "");
        assert_eq!(imported.terminal_shell_custom_args, "");
        assert_eq!(imported.font_size, 21);

        // A shell found through the PATH is taken as it is.
        let mut imported = AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: "nu.exe".to_string(),
            ..AppConfig::default()
        };
        keep_device_settings(&mut imported, current);
        assert_eq!(imported.terminal_shell, "custom");
        assert_eq!(imported.terminal_shell_custom_path, "nu.exe");
    }

    #[test]
    fn missing_key_files_are_listed_once_with_the_profiles_using_them() {
        let present = std::env::current_exe().unwrap();
        let present = present.to_string_lossy();
        let profile = |name: &str, auth: &str, key: &str| {
            serde_json::json!({
                "id": name, "name": name, "connection_type": "ssh", "host": "example.com",
                "port": 22, "username": "root", "auth_method": auth, "private_key_path": key
            })
        };
        let mut through_jump = profile("via-jump", "password", "");
        through_jump["jump_hosts"] = serde_json::json!([{
            "host": "bastion", "port": 22, "username": "root",
            "auth_method": "key", "private_key_path": "/keys/jump"
        }]);
        let mut unused_jump = through_jump.clone();
        unused_jump["name"] = "jump-off".into();
        unused_jump["use_jump_host"] = false.into();
        let profiles: Vec<SavedProfile> = serde_json::from_value(serde_json::json!([
            profile("one", "key", "/keys/shared"),
            profile("two", "key", " /keys/shared "),
            profile("has-key", "key", &present),
            // A path left over from before the profile switched to a password.
            profile("password", "password", "/keys/stale"),
            through_jump,
            unused_jump,
        ]))
        .unwrap();
        assert_eq!(
            missing_key_files(&profiles),
            [
                MissingKeyFile {
                    path: "/keys/jump".to_string(),
                    profiles: vec!["via-jump".to_string()],
                },
                MissingKeyFile {
                    path: "/keys/shared".to_string(),
                    profiles: vec!["one".to_string(), "two".to_string()],
                },
            ]
        );
    }

    #[test]
    fn device_setup_secrets_stay_unless_webdav_settings_travel() {
        assert!(secret_travels("profile-id", false));
        assert!(secret_travels("profile-id:jump", false));
        assert!(secret_travels("Old name:jump", false));
        assert!(!secret_travels("sync:base", true));
        for key in remote::SECRET_KEYS {
            assert!(secret_travels(key, true));
            assert!(!secret_travels(key, false));
        }
    }

    #[test]
    fn backup_and_sync_setup_round_trips_with_its_passwords() {
        let selection = BackupSelection {
            settings: true,
            profiles: true,
            secrets: true,
            ..BackupSelection::default()
        };
        let mut payload = BackupPayload::default();
        payload.webdav_backup = Some(remote::WebDavBackupSettings {
            url: "https://dav.example.com/dav/".to_string(),
            username: "stone".to_string(),
            ..remote::WebDavBackupSettings::default()
        });
        payload.automatic_backup = Some(AutomaticBackupSettings::default());
        payload.sync = Some(crate::sync::SyncSettings {
            enabled: true,
            ..crate::sync::SyncSettings::default()
        });
        for key in remote::SECRET_KEYS {
            payload.secrets.push(MigrationSecretRecord {
                key: key.to_string(),
                password: "dav-secret".to_string(),
            });
        }
        let archive = build_archive_for_version("test", &selection, &payload, Some("password123"))
            .expect("build encrypted archive");
        let path = std::env::temp_dir().join(format!(
            "tterm-backup-setup-test-{}-{}.tterm-backup",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::write(&path, archive).expect("write archive");
        let decoded = decode_archive(&path, Some("password123")).expect("decode archive");
        fs::remove_file(path).expect("remove archive");
        let decoded = decoded.payload.unwrap();
        assert_eq!(
            decoded.webdav_backup.as_ref().unwrap().url,
            "https://dav.example.com/dav/"
        );
        assert_eq!(decoded.automatic_backup.as_ref().unwrap().frequency, "off");
        assert!(decoded.sync.as_ref().unwrap().enabled);
        assert_eq!(decoded.secrets.len(), 2);
    }

    #[test]
    fn frontend_state_is_filtered_by_selected_category() {
        let state = serde_json::json!({
            "customThemes": [{"id":"theme"}],
            "recentCommands": [{"id":"recent"}],
            "recentQuickConnections": [{"host":"example.com"}],
            "sftpColumnWidths": [1, 2],
            "sftpView": {"showHidden": false},
            "untrusted": "ignored"
        });
        let mut selection = BackupSelection::default();
        selection.settings = true;
        let filtered = filter_frontend_state(&selection, Some(state)).unwrap();
        assert!(filtered.get("customThemes").is_none());
        assert!(filtered.get("recentCommands").is_some());
        assert!(filtered.get("recentQuickConnections").is_some());
        assert!(filtered.get("sftpView").is_some());
        assert!(filtered.get("untrusted").is_none());
    }

    #[test]
    fn session_sanitizer_removes_legacy_inline_credentials() {
        let value = serde_json::json!({
            "tabs": [{"connection": {"password": "do-not-export", "host": "example"}}],
            "secretStorage": "memory"
        });
        let cleaned = strip_sensitive_fields(value);
        assert!(cleaned["tabs"][0]["connection"].get("password").is_none());
        assert!(cleaned.get("secretStorage").is_none());
    }

    #[test]
    fn encrypted_archive_round_trips_manifest_and_credentials() {
        let mut selection = BackupSelection::default();
        selection.profiles = true;
        selection.secrets = true;
        let mut payload = BackupPayload::default();
        payload.profiles = Some(serde_json::json!([]));
        payload.secrets.push(MigrationSecretRecord {
            key: "profile-id".to_string(),
            password: "top-secret".to_string(),
        });
        let archive = build_archive_for_version("test", &selection, &payload, Some("password123"))
            .expect("build encrypted archive");
        assert!(!archive
            .windows(b"top-secret".len())
            .any(|value| value == b"top-secret"));

        let path = std::env::temp_dir().join(format!(
            "tterm-backup-test-{}-{}.tterm-backup",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::write(&path, archive).expect("write archive");
        let decoded = decode_archive(&path, Some("password123")).expect("decode archive");
        assert!(decoded.manifest.encrypted);
        assert_eq!(decoded.payload.unwrap().secrets.len(), 1);
        assert!(decode_archive(&path, Some("wrong-password")).is_err());
        fs::remove_file(path).expect("remove archive");
    }
}
