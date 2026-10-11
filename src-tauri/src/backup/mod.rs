use crate::command_library::{CommandRepository, SavedCommand};
use crate::config;
use crate::core::blocking::run_blocking;
use crate::profiles::SavedProfile;
use crate::ssh::{SecretStoreState, VerificationPurpose};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use zeroize::Zeroize;

mod apply;
pub(crate) mod archive;
pub(crate) mod automatic;
mod diff;
mod export;
pub mod remote;
mod snapshot;
pub(crate) mod webdav;

use apply::{
    apply_payload, ensure_selection_available, from_backup_value, import_redirects_webdav,
    missing_key_files, replace_commands,
};
use archive::{build_archive, decode_with_password, validate_payload};
use automatic::{
    create_pre_import_backup, delete_backup_history_entry_blocking, list_backup_history_blocking,
    load_automatic_backup_settings, run_due_automatic_backup_blocking,
    validate_automatic_backup_settings, write_automatic_backup_settings,
};
use diff::{calculate_diff, value_array_len, value_array_len_one};
use export::{collect_payload, validate_export_options};
use snapshot::{
    capture_database_snapshot, capture_file_snapshot, restore_database_snapshot,
    restore_file_snapshot,
};

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
    /// Without a typed password, try the saved WebDAV backup password.
    #[serde(default)]
    pub use_webdav_password: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupImportOptions {
    pub selection: BackupSelection,
    #[serde(default)]
    pub backup_password: Option<String>,
    /// Without a typed password, try the saved WebDAV backup password.
    #[serde(default)]
    pub use_webdav_password: bool,
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
    /// The saved WebDAV backup password opened it.
    pub webdav_password_used: bool,
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
    /// The WebDAV account importing the settings would switch to.
    pub webdav_account: Option<remote::WebDavAccountChange>,
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
pub async fn inspect_backup(
    input: BackupInspectInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<BackupInspectResult, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || inspect_backup_blocking(input, &secret_state)).await
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

fn inspect_backup_blocking(
    input: BackupInspectInput,
    secret_state: &SecretStoreState,
) -> Result<BackupInspectResult, String> {
    let (bundle, webdav_password_used) = decode_with_password(
        Path::new(&input.input_path),
        input.backup_password.as_deref(),
        input.use_webdav_password,
        secret_state,
    )?;
    let payload = bundle.payload.as_ref();
    let diff = payload.map(calculate_diff).transpose()?.unwrap_or_default();
    Ok(BackupInspectResult {
        requires_password: bundle.manifest.encrypted,
        password_verified: payload.is_some(),
        webdav_password_used,
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
    let (mut bundle, _) = decode_with_password(
        Path::new(&input_path),
        options.backup_password.as_deref(),
        options.use_webdav_password,
        secret_state,
    )?;
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

#[tauri::command]
pub async fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
    // The automatic backup directory may be on a slow or network drive.
    run_blocking(list_backup_history_blocking).await
}

#[tauri::command]
pub async fn delete_backup_history_entry(path: String) -> Result<bool, String> {
    run_blocking(move || delete_backup_history_entry_blocking(path)).await
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

#[cfg(test)]
mod tests;
