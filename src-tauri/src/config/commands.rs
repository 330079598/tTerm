//! The Tauri commands for loading and saving the configuration.

use super::{AppConfig, load_config_file};

#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    crate::core::blocking::run_blocking(load_config_file).await
}

#[tauri::command]
pub async fn save_config(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
    config: AppConfig,
) -> Result<(), String> {
    let context = super::save::SaveConfigContext {
        log: crate::connection_context::log(&services),
        close: services.app.close_behavior.clone(),
        tray: crate::app_context::tray(&services),
        integration: services.shared.platform.integration.clone(),
        runtime: services.shared.runtime.clone(),
    };
    crate::core::blocking::run_blocking(move || super::save::save_config(context, config)).await
}
