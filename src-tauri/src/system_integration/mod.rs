//! Tauri commands mapping shared native integration results to the existing IPC DTO.
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tterm_platform::integration::SystemIntegration;
use tterm_platform::native::integration::{IntegrationConfig, IntegrationContext};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    supported: bool,
    installed: bool,
}
impl From<tterm_platform::integration::IntegrationStatus> for IntegrationStatus {
    fn from(value: tterm_platform::integration::IntegrationStatus) -> Self {
        Self {
            supported: value.supported,
            installed: value.installed,
        }
    }
}
pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let context = context(app)?;
    app.manage(Arc::new(context) as Arc<dyn SystemIntegration>);
    Ok(())
}
fn context(app: &AppHandle) -> Result<IntegrationContext, String> {
    Ok(IntegrationContext {
        local_data_dir: crate::app_paths::get(app)?.local_data_dir.clone(),
        identifier: crate::app_identity::get(app)?.identifier.clone(),
        home_dir: app
            .path()
            .home_dir()
            .map_err(|e| format!("Failed to find your home folder: {e}"))?,
    })
}
#[tauri::command]
pub async fn shell_command_status(app: AppHandle) -> Result<IntegrationStatus, String> {
    app.state::<Arc<dyn SystemIntegration>>()
        .shell_status()
        .await
        .map(Into::into)
}
#[tauri::command]
pub async fn set_shell_command(
    app: AppHandle,
    installed: bool,
    admin_prompt: String,
) -> Result<IntegrationStatus, String> {
    app.state::<Arc<dyn SystemIntegration>>()
        .set_shell(installed, admin_prompt)
        .await
        .map(Into::into)
}
#[tauri::command]
pub async fn folder_menu_status(app: AppHandle) -> Result<IntegrationStatus, String> {
    app.state::<Arc<dyn SystemIntegration>>()
        .folder_status()
        .await
        .map(Into::into)
}
#[tauri::command]
pub async fn set_folder_menu(app: AppHandle, installed: bool) -> Result<IntegrationStatus, String> {
    let language = crate::config::load_config_file()
        .map(|c| c.language)
        .unwrap_or_default();
    let label = if language == "zh" {
        "在 tTerm 中打开"
    } else {
        "Open in tTerm"
    };
    app.state::<Arc<dyn SystemIntegration>>()
        .set_folder(installed, label.into())
        .await
        .map(Into::into)
}
pub fn apply_config(app: &AppHandle, cfg: &crate::config::AppConfig) {
    match context(app) {
        Ok(context) => tterm_platform::native::integration::apply_config(
            &context,
            &IntegrationConfig {
                language: cfg.language.clone(),
            },
        ),
        Err(error) => eprintln!("Failed to update system integration: {error}"),
    }
}
#[cfg(target_os = "macos")]
pub fn tab_bin_dir() -> Option<&'static std::path::Path> {
    tterm_platform::native::integration::tab_bin_dir()
}
#[cfg(windows)]
pub fn run_installer_request() -> Option<i32> {
    tterm_platform::native::integration::run_installer_request()
}
