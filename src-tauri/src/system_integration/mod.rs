//! Tauri commands mapping shared native integration results to the existing IPC DTO.
use serde::Serialize;
use std::sync::Arc;
use tterm_platform::integration::SystemIntegration;
use tterm_platform::native::integration::IntegrationContext;

pub(crate) mod commands;

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
/// Records either the integration capability or its error without aborting startup.
pub fn initialize(
    context: Result<IntegrationContext, String>,
) -> Result<Arc<dyn SystemIntegration>, String> {
    let integration = context
        .map(|context| Arc::new(context) as Arc<dyn SystemIntegration>)
        .map_err(|error| format!("System integration is unavailable: {error}. Check your home folder and restart tTerm."));
    if let Err(error) = &integration {
        eprintln!("{error}");
    }
    integration
}

fn integration(
    state: &Result<Arc<dyn SystemIntegration>, String>,
) -> Result<Arc<dyn SystemIntegration>, String> {
    state.clone()
}

async fn set_folder_menu_service(
    integration: Arc<dyn SystemIntegration>,
    installed: bool,
) -> Result<IntegrationStatus, String> {
    let language = crate::core::blocking::run_blocking(|| {
        Ok(crate::config::load_config_file()
            .map(|config| config.language)
            .unwrap_or_default())
    })
    .await?;
    integration
        .set_folder(installed, language)
        .await
        .map(Into::into)
}
pub async fn apply_config(
    state: &Result<Arc<dyn SystemIntegration>, String>,
    cfg: &crate::config::AppConfig,
) {
    match integration(state) {
        Ok(integration) => {
            if let Err(error) = integration.refresh(cfg.language.clone()).await {
                eprintln!("Failed to update system integration: {error}");
            }
        }
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
    #[test]
    fn failed_initialization_keeps_the_app_running_and_commands_report_the_cause() {
        let state = super::initialize(Err("Failed to find your home folder".into()));
        let error = super::integration(&state).err().unwrap();
        assert!(error.contains("Failed to find your home folder"));
        assert!(error.contains("restart tTerm"));
        assert!(state.is_err());
    }
}
