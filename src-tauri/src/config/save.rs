//! Applies saved settings to the services that cache configuration.
use std::sync::Arc;
use tterm_core::session_log::SessionLogState;
use tterm_core::{config::AppConfig, config::load_config_file, config::save_config_file};

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
