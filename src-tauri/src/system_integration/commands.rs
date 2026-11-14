//! Tauri commands for the shell command and folder menu integration.

use std::sync::Arc;

use tterm_core::system_integration::IntegrationStatus;

#[tauri::command]
pub async fn shell_command_status(
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> Result<IntegrationStatus, String> {
    tterm_core::system_integration::integration(&services.shared.platform.integration)?
        .shell_status()
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn set_shell_command(
    installed: bool,
    admin_prompt: String,
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> Result<IntegrationStatus, String> {
    tterm_core::system_integration::integration(&services.shared.platform.integration)?
        .set_shell(installed, admin_prompt)
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn folder_menu_status(
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> Result<IntegrationStatus, String> {
    tterm_core::system_integration::integration(&services.shared.platform.integration)?
        .folder_status()
        .await
        .map(Into::into)
}

#[tauri::command]
pub async fn set_folder_menu(
    installed: bool,
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> Result<IntegrationStatus, String> {
    tterm_core::system_integration::set_folder_menu_service(
        tterm_core::system_integration::integration(&services.shared.platform.integration)?,
        installed,
    )
    .await
}
