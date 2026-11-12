//! The Tauri commands for loading and saving the configuration.

use super::{AppConfig, load_config_file};
use std::sync::Arc;

#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    crate::core::blocking::run_blocking(load_config_file).await
}

#[tauri::command]
pub async fn save_config(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
    config: AppConfig,
) -> Result<(), String> {
    let log_state = services.app.session_log.clone();
    let config =
        crate::core::blocking::run_blocking(move || super::save::save_config(log_state, config))
            .await?;
    crate::background::apply_config(
        &services.app.close_behavior,
        &crate::app_context::tray(&services),
        &config,
    );
    services
        .shared
        .runtime
        .block_on(crate::system_integration::apply_config(
            &services.shared.platform.integration,
            &config,
        ));
    services
        .app
        .session_log
        .apply_config(&crate::connection_context::log(&services), &config)
}
