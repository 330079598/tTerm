//! Applies saved settings to the services that cache configuration.
use super::{AppConfig, load_config_file, save_config_file};
use crate::{
    background::{CloseBehaviorState, tray::TrayContext},
    session_log::LogContext,
};
use std::sync::Arc;
use tterm_platform::integration::SystemIntegration;

pub(super) struct SaveConfigContext {
    pub log: LogContext,
    pub close: Arc<CloseBehaviorState>,
    pub tray: TrayContext,
    pub integration: Result<Arc<dyn SystemIntegration>, String>,
    pub runtime: tokio::runtime::Handle,
}

pub(super) fn save_config(context: SaveConfigContext, mut config: AppConfig) -> Result<(), String> {
    // Only the secret store commands move keys when the storage mode changes.
    config.secret_storage_mode = load_config_file()?.secret_storage_mode;
    context.log.state.validate_config(&config)?;
    save_config_file(&config)?;
    crate::sftp::internal::api::apply_bandwidth_limits(&config);
    crate::background::apply_config(&context.close, &context.tray, &config);
    context
        .runtime
        .block_on(crate::system_integration::apply_config(
            &context.integration,
            &config,
        ));
    context.log.state.apply_config(&context.log, &config)
}
