//! Applies saved settings to the services that cache configuration.
use super::{AppConfig, load_config_file, save_config_file};
use crate::session_log::SessionLogState;
use std::sync::Arc;

pub(super) fn save_config(
    log_state: Arc<SessionLogState>,
    mut config: AppConfig,
) -> Result<AppConfig, String> {
    // Only the secret store commands move keys when the storage mode changes.
    config.secret_storage_mode = load_config_file()?.secret_storage_mode;
    log_state.validate_config(&config)?;
    save_config_file(&config)?;
    crate::sftp::internal::api::apply_bandwidth_limits(&config);
    Ok(config)
}
