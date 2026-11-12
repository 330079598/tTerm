//! Domain operations behind the IPC entry points.
use super::state;
use super::{
    SYNC_LOCK, SyncOutcome, SyncSettings, SyncStatus, set_last_error, status, sync, write_settings,
};
use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::types::SecretStoreState;
use crate::tunnel::manager::TunnelManager;
use serde_json::Value;

pub(super) async fn get_sync_status() -> Result<SyncStatus, String> {
    run_blocking(status).await
}

pub(super) async fn save_sync_settings(settings: SyncSettings) -> Result<SyncStatus, String> {
    run_blocking(move || {
        settings.validate()?;
        crate::db::write(|transaction| write_settings(transaction, &settings))?;
        status()
    })
    .await
}

pub(super) async fn reset_sync() -> Result<SyncStatus, String> {
    let _guard = SYNC_LOCK.lock().await;
    run_blocking(|| {
        crate::db::write(|transaction| state::clear(transaction))?;
        set_last_error(None);
        status()
    })
    .await
}

pub(super) async fn run_sync(
    app_version: String,
    frontend_state: Option<Value>,
    check_remote: bool,
    secret_state: &SecretStoreState,
    tunnels: &TunnelManager,
) -> Result<SyncOutcome, String> {
    let Ok(_guard) = SYNC_LOCK.try_lock() else {
        return Ok(SyncOutcome::skipped("busy"));
    };
    let result = sync(
        &app_version,
        frontend_state,
        check_remote,
        secret_state,
        tunnels,
    )
    .await;
    match &result {
        Ok(outcome) if matches!(outcome.state, "synced" | "unchanged") => set_last_error(None),
        Ok(_) => {}
        Err(error) => set_last_error(Some(error.clone())),
    }
    result
}
