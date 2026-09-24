use crate::command_library::{CommandRepository, SavedCommand};
use crate::config::{self, AppConfig};
use crate::core::blocking::run_blocking;
use crate::profiles::SavedProfile;
use crate::session::SessionData;
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::store::KnownHostStore;
use crate::ssh::SecretStoreState;
use crate::tunnel::TunnelRule;
use aes_gcm::aead::{Aead, KeyInit, Payload as AeadPayload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, State};
use zeroize::Zeroize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const FORMAT_NAME: &str = "tterm-backup";
const FORMAT_VERSION: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const PAYLOAD_ENTRY: &str = "payload.json";
const ENCRYPTED_PAYLOAD_ENTRY: &str = "payload.enc";
const MAX_BACKUP_SIZE: u64 = 128 * 1024 * 1024;
const MAX_PAYLOAD_SIZE: u64 = 64 * 1024 * 1024;
const BACKUP_AAD: &[u8] = b"tterm-backup-v1";

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
    #[serde(default = "default_secret_destination")]
    pub secret_destination: String,
}

fn default_conflict_strategy() -> String {
    "merge".to_string()
}

fn default_secret_destination() -> String {
    "auto".to_string()
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
    pub secret_destination: Option<String>,
    pub frontend_state: Option<Value>,
    pub pre_import_backup_path: String,
    pub requires_restart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticBackupSettings {
    pub frequency: String,
    pub directory: String,
    pub retention_count: u16,
    pub selection: BackupSelection,
    #[serde(default)]
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

    let mut hybrid_snapshot = None;
    let current_mode = config::load_config_file()?.secret_storage_mode;
    let destination = if options.selection.secrets {
        Some(resolve_secret_destination(
            secret_state,
            &options.secret_destination,
            &current_mode,
        )?)
    } else {
        None
    };
    // Capture the original application config before hybrid initialization changes it.
    let config_snapshot = if destination.is_some() && !options.selection.settings {
        let config_path = config::get_config_path()?.join("config.json");
        Some((
            config_path.clone(),
            fs::read(&config_path)
                .map_err(|error| format!("Failed to snapshot secret storage config: {error}"))?,
        ))
    } else {
        None
    };
    // A device already in hybrid mode with its vault unlocked needs no setup;
    // checking its keyring master would only cost a keychain prompt.
    let already_hybrid = current_mode == "hybrid" && secret_state.vault_unlocked()?;
    if destination.as_deref() == Some("hybrid") && !already_hybrid {
        if !secret_state.vault_unlocked()? {
            let password = options
                .backup_password
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| {
                    "A backup password is required to initialize the target vault.".to_string()
                })?;
            hybrid_snapshot = Some(secret_state.prepare_migration_hybrid(&app, password)?);
        } else {
            // An already-unlocked vault can only be switched to hybrid if it
            // already has a master password in the system credential store.
            // The import password is the backup password, not necessarily the
            // existing vault password.
            if !secret_state.hybrid_master_available()? {
                return Err("Hybrid migration requires the existing vault password to be configured for system auto-unlock.".to_string());
            }
        }
    }

    let pre_import_backup_path =
        match create_pre_import_backup(&app, &options.selection, secret_state) {
            Ok(path) => path,
            Err(error) => {
                if let Some(snapshot) = hybrid_snapshot.take() {
                    let _ = secret_state.rollback_migration_hybrid(snapshot);
                }
                return Err(error);
            }
        };
    let file_snapshot = match capture_file_snapshot(&options.selection, payload) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Some(snapshot) = hybrid_snapshot.take() {
                let _ = secret_state.rollback_migration_hybrid(snapshot);
            }
            return Err(error);
        }
    };
    let database_snapshot = match capture_database_snapshot(&options.selection) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Some(snapshot) = hybrid_snapshot.take() {
                let _ = secret_state.rollback_migration_hybrid(snapshot);
            }
            return Err(error);
        }
    };
    let command_snapshot = if options.selection.command_library {
        let database = match crate::db::get() {
            Ok(database) => database,
            Err(error) => {
                if let Some(snapshot) = hybrid_snapshot.take() {
                    let _ = secret_state.rollback_migration_hybrid(snapshot);
                }
                return Err(error);
            }
        };
        match CommandRepository::new(database).list() {
            Ok(commands) => Some(commands),
            Err(error) => {
                if let Some(snapshot) = hybrid_snapshot.take() {
                    let _ = secret_state.rollback_migration_hybrid(snapshot);
                }
                return Err(error);
            }
        }
    } else {
        None
    };
    let secret_snapshot = if let Some(destination) = destination.as_deref() {
        match capture_secret_snapshot(&app, secret_state, &payload.secrets, destination) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                if let Some(snapshot) = hybrid_snapshot.take() {
                    let _ = secret_state.rollback_migration_hybrid(snapshot);
                }
                return Err(error);
            }
        }
    } else {
        Vec::new()
    };

    let apply_result = apply_payload(
        &app,
        payload,
        &options,
        destination.as_deref(),
        secret_state,
    );
    let (profiles_imported, commands_imported, secrets_imported) = match apply_result {
        Ok(counts) => counts,
        Err(error) => {
            let mut rollback_errors = Vec::new();
            let had_hybrid_snapshot = hybrid_snapshot.is_some();
            let mut hybrid_rollback_failed = false;
            if let Some(snapshot) = hybrid_snapshot.take() {
                match secret_state.rollback_migration_hybrid(snapshot) {
                    Ok(()) => {}
                    Err(err) => {
                        hybrid_rollback_failed = true;
                        rollback_errors.push(err);
                    }
                }
            }
            if let Err(err) = restore_file_snapshot(&file_snapshot) {
                rollback_errors.push(err);
            }
            if let Err(err) = restore_database_snapshot(&database_snapshot) {
                rollback_errors.push(err);
            }
            if !had_hybrid_snapshot || hybrid_rollback_failed {
                if let Some((path, bytes)) = config_snapshot.as_ref() {
                    if let Err(err) = config::atomic_write_private(path, bytes) {
                        rollback_errors
                            .push(format!("Failed to restore secret storage config: {err}"));
                    }
                }
            }
            if let Some(commands) = command_snapshot.as_ref() {
                if let Err(err) = replace_commands(commands) {
                    rollback_errors.push(err);
                }
            }
            if !had_hybrid_snapshot {
                if let Some(destination) = destination.as_deref() {
                    if let Err(err) =
                        restore_secret_snapshot(&app, secret_state, &secret_snapshot, destination)
                    {
                        rollback_errors.push(err);
                    }
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
    let imported_secrets = destination.is_some();
    Ok(BackupImportResult {
        profiles_imported,
        commands_imported,
        secrets_imported,
        secret_destination: destination,
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
pub fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    load_automatic_backup_settings()
}

#[tauri::command]
pub fn save_automatic_backup_settings(
    settings: AutomaticBackupSettings,
) -> Result<AutomaticBackupSettings, String> {
    validate_automatic_backup_settings(&settings)?;
    save_automatic_backup_settings_file(&settings)?;
    Ok(settings)
}

fn run_due_automatic_backup_blocking(
    app: AppHandle,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: &SecretStoreState,
) -> Result<Option<BackupExportResult>, String> {
    let mut settings = load_automatic_backup_settings()?;
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
    settings.last_backup_at = Some(Utc::now().timestamp_millis());
    save_automatic_backup_settings_file(&settings)?;
    enforce_retention(&directory, settings.retention_count as usize)?;
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
            } else if file_name.starts_with("pre-import-") {
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
    if selection.secrets {
        let status = secret_state.get_status()?;
        if matches!(status.storage_mode.as_str(), "vault" | "hybrid") && !status.vault_unlocked {
            return Err("Unlock the app vault before exporting all saved passwords.".to_string());
        }
    }
    let mut payload = BackupPayload {
        config: read_selected_json(selection.settings, "config.json")?,
        profiles: read_selected_data(selection.profiles, crate::profiles::list_saved_profiles)?,
        profile_groups: read_selected_data(
            selection.profiles,
            crate::profiles::configured_profile_groups,
        )?,
        tunnels: read_selected_data(selection.profiles, crate::tunnel::list_tunnel_rules)?,
        session: read_selected_json(selection.session, "session.json")?,
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
        frontend_state: filter_frontend_state(selection, frontend_state),
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

    if selection.secrets {
        let mut keys = crate::profiles::saved_secret_keys()?;
        keys.sort();
        keys.dedup();
        let passwords = secret_state.get_passwords_for_migration(&keys)?;
        for key in keys {
            if let Some(password) = passwords.get(&key) {
                payload.secrets.push(MigrationSecretRecord {
                    key,
                    password: (**password).clone(),
                });
            }
        }
        // Older installations may still have the legacy plaintext store when the
        // system keyring was unavailable during startup. Include it in the encrypted
        // migration instead of silently dropping those accounts.
        let exported_keys = payload
            .secrets
            .iter()
            .map(|secret| secret.key.clone())
            .collect::<HashSet<_>>();
        for record in crate::ssh::load_legacy_password_store()?.profiles {
            if !record.password.is_empty() && !exported_keys.contains(&record.profile_name) {
                payload.secrets.push(MigrationSecretRecord {
                    key: record.profile_name,
                    password: record.password,
                });
            }
        }
    }
    validate_payload(&payload)?;
    Ok(payload)
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

fn filter_frontend_state(selection: &BackupSelection, state: Option<Value>) -> Option<Value> {
    let source = state?.as_object()?.clone();
    let mut filtered = serde_json::Map::new();
    if selection.themes {
        if let Some(value) = source.get("customThemes") {
            filtered.insert("customThemes".to_string(), value.clone());
        }
    }
    if selection.settings {
        for key in ["recentCommands", "sftpColumnWidths"] {
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
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let mut key_bytes = derive_key(password, &salt, memory_kib, iterations, parallelism)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
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
    if salt.len() != 16 || nonce.len() != 12 {
        return Err("Backup encryption metadata is invalid.".to_string());
    }
    let mut key_bytes = derive_key(
        password,
        &salt,
        crypto.memory_kib,
        crypto.iterations,
        crypto.parallelism,
    )?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let result = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            AeadPayload {
                msg: ciphertext,
                aad: BACKUP_AAD,
            },
        )
        .map_err(|_| "Unable to decrypt backup. Check the backup password.".to_string());
    key_bytes.zeroize();
    result
}

fn derive_key(
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

fn resolve_secret_destination(
    state: &SecretStoreState,
    requested: &str,
    current_mode: &str,
) -> Result<String, String> {
    choose_secret_destination(
        requested,
        current_mode,
        || state.keyring_available(),
        state.vault_unlocked()?,
    )
}

/// Picks where imported passwords go. `auto` keeps the device's current
/// storage mode, so an import never moves passwords out of the vault into the
/// system store (one keychain prompt per password on macOS) or switches the
/// mode behind the user's back. The keyring is only probed when needed.
fn choose_secret_destination(
    requested: &str,
    current_mode: &str,
    keyring_available: impl Fn() -> Result<bool, String>,
    vault_unlocked: bool,
) -> Result<String, String> {
    let requested = match (requested, current_mode) {
        ("auto", "system" | "vault" | "hybrid") => current_mode,
        (requested, _) => requested,
    };
    match requested {
        "system" if keyring_available()? => Ok("system".to_string()),
        "system" => Err("System credential store is unavailable.".to_string()),
        "vault" if vault_unlocked => Ok("vault".to_string()),
        "vault" => Err("Unlock the app vault before importing passwords.".to_string()),
        "hybrid" if keyring_available()? => Ok("hybrid".to_string()),
        "hybrid" => Err("Hybrid credential storage requires the system credential store.".to_string()),
        // Devices in `auto` or `memory` mode: the system store when available,
        // as older versions did, else an unlocked vault.
        "auto" if keyring_available()? => Ok("system".to_string()),
        "auto" if vault_unlocked => Ok("vault".to_string()),
        "auto" => Err(
            "No persistent credential store is ready. Enable system credentials or unlock the app vault."
                .to_string(),
        ),
        _ => Err("Secret destination must be auto, system, vault, or hybrid.".to_string()),
    }
}

fn apply_payload(
    app: &AppHandle,
    payload: &BackupPayload,
    options: &BackupImportOptions,
    destination: Option<&str>,
    secret_state: &SecretStoreState,
) -> Result<(usize, usize, usize), String> {
    let directory = config::ensure_config_dir()?;
    if options.selection.settings {
        if let Some(value) = payload.config.as_ref() {
            let mut imported = serde_json::from_value::<AppConfig>(value.clone())
                .map_err(|error| format!("Invalid settings: {error}"))?;
            // Secret backends are device-local. Never let a Windows `system` or
            // another source-device mode overwrite the destination policy.
            let current = config::load_config_file()?;
            imported.secret_storage_mode = current.secret_storage_mode;
            imported.secret_vault_enabled = current.secret_vault_enabled;
            imported.prompt_unlock_vault_on_startup = current.prompt_unlock_vault_on_startup;
            config::save_config_file(&imported)?;
        }
    }
    if let Some(destination) = destination {
        let mut imported = config::load_config_file()?;
        imported.secret_storage_mode = destination.to_string();
        if matches!(destination, "vault" | "hybrid") {
            imported.secret_vault_enabled = true;
        }
        config::save_config_file(&imported)?;
    }

    let profiles_imported =
        crate::db::write(|transaction| apply_database_payload(transaction, payload, options))?;
    if options.selection.session {
        write_optional_json(&directory.join("session.json"), payload.session.as_ref())?;
    }
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

    let secrets_imported = if let Some(destination) = destination {
        for secret in &payload.secrets {
            secret_state.write_migration_destination(
                app,
                &secret.key,
                &secret.password,
                destination,
            )?;
        }
        payload.secrets.len()
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
    known_hosts: Option<KnownHostStore>,
    sftp_directories: Option<SftpDirectoryStore>,
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
    if let Some(store) = snapshot.known_hosts.as_ref() {
        crate::ssh::store::replace_known_hosts(transaction, store)?;
    }
    if let Some(store) = snapshot.sftp_directories.as_ref() {
        crate::sftp::store::replace_sftp_directories(transaction, store)?;
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

fn write_optional_json(path: &Path, value: Option<&Value>) -> Result<(), String> {
    if let Some(value) = value {
        write_json_value(path, value)?;
    }
    Ok(())
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Failed to serialize '{}': {error}", path.display()))?;
    config::atomic_write(path, bytes)
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
    if selection.session {
        names.push("session.json");
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

fn capture_secret_snapshot(
    app: &AppHandle,
    state: &SecretStoreState,
    secrets: &[MigrationSecretRecord],
    destination: &str,
) -> Result<Vec<(String, Option<String>)>, String> {
    secrets
        .iter()
        .map(|secret| {
            state
                .read_migration_destination(app, &secret.key, destination)
                .map(|value| (secret.key.clone(), value))
        })
        .collect()
}

fn restore_secret_snapshot(
    app: &AppHandle,
    state: &SecretStoreState,
    snapshot: &[(String, Option<String>)],
    destination: &str,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for (key, value) in snapshot {
        let result = if let Some(password) = value {
            state.write_migration_destination(app, key, password, destination)
        } else {
            state.delete_migration_destination(app, key, destination)
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

fn automatic_backup_settings_path() -> Result<PathBuf, String> {
    Ok(config::ensure_config_dir()?.join("backup_settings.json"))
}

fn load_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    let path = automatic_backup_settings_path()?;
    if !path.exists() {
        return Ok(AutomaticBackupSettings::default());
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Failed to read automatic backup settings: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse automatic backup settings: {error}"))
}

fn save_automatic_backup_settings_file(settings: &AutomaticBackupSettings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Failed to serialize automatic backup settings: {error}"))?;
    config::atomic_write(&automatic_backup_settings_path()?, bytes)
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

fn enforce_retention(directory: &Path, retention: usize) -> Result<(), String> {
    let mut backups = fs::read_dir(directory)
        .map_err(|error| format!("Failed to read automatic backup directory: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("automatic-") || !name.ends_with(".tterm-backup") {
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
            secret_destination: default_secret_destination(),
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
    fn auto_secret_destination_keeps_the_current_storage_mode() {
        let keyring = |available: bool| move || Ok(available);
        let no_probe = || -> Result<bool, String> { panic!("keyring must not be probed") };

        // A hybrid or vault device keeps its passwords in the vault.
        assert_eq!(
            choose_secret_destination("auto", "hybrid", keyring(true), true).unwrap(),
            "hybrid"
        );
        assert_eq!(
            choose_secret_destination("auto", "vault", no_probe, true).unwrap(),
            "vault"
        );
        assert!(choose_secret_destination("auto", "vault", no_probe, false)
            .unwrap_err()
            .contains("Unlock the app vault"));
        assert_eq!(
            choose_secret_destination("auto", "system", keyring(true), false).unwrap(),
            "system"
        );
        // Devices without a fixed mode keep the old preference.
        assert_eq!(
            choose_secret_destination("auto", "auto", keyring(true), true).unwrap(),
            "system"
        );
        assert_eq!(
            choose_secret_destination("auto", "memory", keyring(false), true).unwrap(),
            "vault"
        );
        // An explicit choice wins over the current mode.
        assert_eq!(
            choose_secret_destination("system", "hybrid", keyring(true), true).unwrap(),
            "system"
        );
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
    fn frontend_state_is_filtered_by_selected_category() {
        let state = serde_json::json!({
            "customThemes": [{"id":"theme"}],
            "recentCommands": [{"id":"recent"}],
            "sftpColumnWidths": [1, 2],
            "untrusted": "ignored"
        });
        let mut selection = BackupSelection::default();
        selection.settings = true;
        let filtered = filter_frontend_state(&selection, Some(state)).unwrap();
        assert!(filtered.get("customThemes").is_none());
        assert!(filtered.get("recentCommands").is_some());
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
