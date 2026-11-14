//! Domain operations behind the IPC entry points.
use crate::{
    backup::AutomaticBackupSettings, backup::BackupExportOptions, backup::BackupExportResult,
    backup::BackupHistoryEntry, backup::BackupImportOptions, backup::BackupImportResult,
    backup::BackupInspectInput, backup::BackupInspectResult, backup::export_backup_blocking,
    backup::import_backup_blocking, backup::inspect_backup_blocking,
};
use crate::{
    backup::automatic::delete_backup_history_entry_blocking,
    backup::automatic::list_backup_history_blocking,
    backup::automatic::load_automatic_backup_settings,
    backup::automatic::run_due_automatic_backup_blocking,
    backup::automatic::validate_automatic_backup_settings,
    backup::automatic::write_automatic_backup_settings,
};
use serde_json::Value;
use tterm_core::core::blocking::run_blocking;
use tterm_core::ssh::secret_store::types::SecretStoreState;

pub(super) async fn export_backup(
    app_version: String,
    output_path: String,
    options: BackupExportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupExportResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || export_backup_blocking(app_version, output_path, options, &secret_state))
        .await
}

pub(super) async fn inspect_backup(
    input: BackupInspectInput,
    secret_state: &SecretStoreState,
) -> Result<BackupInspectResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || inspect_backup_blocking(input, &secret_state)).await
}

pub(super) async fn import_backup(
    app_version: String,
    input_path: String,
    options: BackupImportOptions,
    secret_state: &SecretStoreState,
) -> Result<BackupImportResult, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || import_backup_blocking(app_version, input_path, options, &secret_state))
        .await
}

pub(super) async fn run_due_automatic_backup(
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

pub(super) async fn get_automatic_backup_settings() -> Result<AutomaticBackupSettings, String> {
    run_blocking(load_automatic_backup_settings).await
}

pub(super) async fn save_automatic_backup_settings(
    settings: AutomaticBackupSettings,
) -> Result<AutomaticBackupSettings, String> {
    run_blocking(move || {
        validate_automatic_backup_settings(&settings)?;
        tterm_core::db::write(|transaction| {
            write_automatic_backup_settings(transaction, &settings)
        })?;
        load_automatic_backup_settings()
    })
    .await
}

pub(super) async fn list_backup_history() -> Result<Vec<BackupHistoryEntry>, String> {
    // The automatic backup directory may be on a slow or network drive.
    run_blocking(list_backup_history_blocking).await
}

pub(super) async fn delete_backup_history_entry(path: String) -> Result<bool, String> {
    run_blocking(move || delete_backup_history_entry_blocking(path)).await
}
