use std::sync::Arc;

use crate::ssh::secret_store::types::SecretStoreState;
use tterm_core::{events::EventSink, prompts::PromptBroker};
use tterm_platform::{
    clipboard::Clipboard, drag::FileDrag, notifications::AppBadge, open::OpenTargets,
    process::ProcessControl, tray::Tray, updates::Updates, verification::UserVerification,
    window::MainWindow,
};

pub struct SharedServices {
    pub paths: Arc<tterm_core::paths::AppPaths>,
    pub identity: Arc<tterm_core::identity::AppIdentity>,
    pub events: Arc<dyn EventSink>,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub runtime: tokio::runtime::Handle,
    pub platform: PlatformServices,
}

pub struct PlatformServices {
    pub notifications: Arc<dyn tterm_platform::notifications::Notifications>,
    pub integration: Result<Arc<dyn tterm_platform::integration::SystemIntegration>, String>,
    pub badge: Arc<dyn AppBadge>,
    pub tray: Arc<dyn Tray>,
    pub file_drag: Arc<dyn FileDrag>,
    pub main_window: Arc<dyn MainWindow>,
    pub process: Arc<dyn ProcessControl>,
    pub verification: Arc<dyn UserVerification>,
    pub updates: Arc<dyn Updates>,
    pub open: Arc<dyn OpenTargets>,
    pub clipboard: Arc<dyn Clipboard>,
}
