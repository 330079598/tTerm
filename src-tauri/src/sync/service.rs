//! Domain operations behind the IPC entry points.
use crate::sync::state;
use crate::{
    sync::SYNC_LOCK, sync::SyncOutcome, sync::SyncSettings, sync::SyncStatus, sync::set_last_error,
    sync::status, sync::sync, sync::write_settings,
};
use serde_json::Value;
use tterm_core::core::blocking::run_blocking;
use tterm_core::ssh::secret_store::types::SecretStoreState;
use tterm_core::tunnel::manager::TunnelManager;

pub(super) async fn get_sync_status() -> Result<SyncStatus, String> {
    run_blocking(status).await
}

pub(super) async fn save_sync_settings(settings: SyncSettings) -> Result<SyncStatus, String> {
    run_blocking(move || {
        settings.validate()?;
        tterm_core::db::write(|transaction| write_settings(transaction, &settings))?;
        status()
    })
    .await
}

pub(super) async fn reset_sync() -> Result<SyncStatus, String> {
    let _guard = SYNC_LOCK.lock().await;
    run_blocking(|| {
        tterm_core::db::write(|transaction| state::clear(transaction))?;
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
