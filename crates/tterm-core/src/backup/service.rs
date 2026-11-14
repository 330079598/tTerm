//! Domain operations behind the IPC entry points.
use crate::backup::AutomaticBackupSettings;
use crate::backup::BackupExportOptions;
use crate::backup::BackupExportResult;
use crate::backup::BackupHistoryEntry;
use crate::backup::BackupImportOptions;
use crate::backup::BackupImportResult;
use crate::backup::BackupInspectInput;
use crate::backup::BackupInspectResult;
use crate::backup::automatic::delete_backup_history_entry_blocking;
use crate::backup::automatic::list_backup_history_blocking;
use crate::backup::automatic::load_automatic_backup_settings;
use crate::backup::automatic::run_due_automatic_backup_blocking;
use crate::backup::automatic::validate_automatic_backup_settings;
use crate::backup::automatic::write_automatic_backup_settings;
use crate::backup::export_backup_blocking;
use crate::backup::import_backup_blocking;
use crate::backup::inspect_backup_blocking;
use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::types::SecretStoreState;
use serde_json::Value;

pub async fn export_backup(
    app_version: String,
    output_path: String,
    options: BackupExportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupExportResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || export_backup_blocking(app_version, output_path, options, &secret_state))
        .await
}

pub async fn inspect_backup(
    input: BackupInspectInput,
    secret_state: &SecretStoreState,
) -> Result<BackupInspectResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || inspect_backup_blocking(input, &secret_state)).await
}

pub async fn import_backup(
    app_version: String,
    input_path: String,
    options: BackupImportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupImportResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || import_backup_blocking(app_version, input_path, options, &secret_state))
        .await
}

pub async fn run_due_automatic_backup(
    app_version: String,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: &SecretStoreState,
) -> Result<Option<BackupExportResult>, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        run_due_automatic_backup_blocking(app_version, frontend_state, force, &secret_state)
    })
    .await
}

pub async fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    run_blocking(load_automatic_backup_settings).await
}

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

pub async fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
    // The automatic backup directory may be on a slow or network drive.
    run_blocking(list_backup_history_blocking).await
}

pub async fn delete_backup_history_entry(path: String) -> Result<bool, String> {
    run_blocking(move || delete_backup_history_entry_blocking(path)).await
}
