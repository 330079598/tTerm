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
struct IntegrationState(Result<Arc<dyn SystemIntegration>, String>);

/// Records either the integration capability or its error without aborting startup.
pub fn initialize(app: &AppHandle) {
    initialize_with(app, context(app));
}

fn initialize_with<R: tauri::Runtime>(
    app: &AppHandle<R>,
    context: Result<IntegrationContext, String>,
) {
    let integration = context
        .map(|context| Arc::new(context) as Arc<dyn SystemIntegration>)
        .map_err(|error| {
            format!("System integration is unavailable: {error}. Check your home folder and restart tTerm.")
        });
    if let Err(error) = &integration {
        eprintln!("{error}");
    }
    app.manage(IntegrationState(integration));
}

fn integration<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<Arc<dyn SystemIntegration>, String> {
    app.try_state::<IntegrationState>()
        .ok_or_else(|| {
            "System integration is not ready yet. Restart tTerm and try again.".to_string()
        })?
        .0
        .clone()
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
    integration(&app)?.shell_status().await.map(Into::into)
}
#[tauri::command]
pub async fn set_shell_command(
    app: AppHandle,
    installed: bool,
    admin_prompt: String,
) -> Result<IntegrationStatus, String> {
    integration(&app)?
        .set_shell(installed, admin_prompt)
        .await
        .map(Into::into)
}
#[tauri::command]
pub async fn folder_menu_status(app: AppHandle) -> Result<IntegrationStatus, String> {
    integration(&app)?.folder_status().await.map(Into::into)
}
#[tauri::command]
pub async fn set_folder_menu(app: AppHandle, installed: bool) -> Result<IntegrationStatus, String> {
    let language = crate::core::blocking::run_blocking(|| {
        Ok(crate::config::load_config_file()
            .map(|config| config.language)
            .unwrap_or_default())
    })
    .await?;
    integration(&app)?
        .set_folder(installed, language)
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

#[cfg(test)]
mod tests {
    use tauri::Manager;

    #[test]
    fn failed_initialization_keeps_the_app_running_and_commands_report_the_cause() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        super::initialize_with(app.handle(), Err("Failed to find your home folder".into()));
        let error = super::integration(app.handle()).err().unwrap();
        assert!(error.contains("Failed to find your home folder"));
        assert!(error.contains("restart tTerm"));
        assert!(app
            .handle()
            .try_state::<super::IntegrationState>()
            .is_some());
    }
}
