//! Thin adapters for Tauri's plugins and window runtime.
mod badge;
mod clipboard;
mod open;
mod tray;
mod tray_native;
mod updates;
mod verification;
mod window;

use std::sync::Arc;
use tauri::AppHandle;
use tterm_platform::window::WindowHost;

type Notifications = Arc<dyn tterm_platform::notifications::Notifications>;
type Integration = Result<Arc<dyn tterm_platform::integration::SystemIntegration>, String>;
/// Builds adapters once; startup supplies notifications and integration in their original order.
pub fn initialize(
    app: &AppHandle,
    complete: impl FnOnce(
        Arc<dyn tterm_platform::window::MainWindow>,
        Arc<dyn tterm_platform::notifications::AppBadge>,
    ) -> (Notifications, Integration),
) -> crate::services::shared::PlatformServices {
    let main_window: Arc<dyn tterm_platform::window::MainWindow> =
        Arc::new(window::TauriWindow(app.clone()));
    let window_host: Arc<dyn WindowHost> = Arc::new(window::TauriWindow(app.clone()));
    let badge: Arc<dyn tterm_platform::notifications::AppBadge> =
        Arc::new(badge::TauriBadge(app.clone()));
    let tray: Arc<dyn tterm_platform::tray::Tray> = Arc::new(tray::TauriTray::new(app.clone()));
    let file_drag: Arc<dyn tterm_platform::drag::FileDrag> =
        Arc::new(tterm_platform::native::drag::NativeFileDrag(window_host));
    let process: Arc<dyn tterm_platform::process::ProcessControl> =
        Arc::new(window::TauriProcess(app.clone()));
    let verification: Arc<dyn tterm_platform::verification::UserVerification> =
        Arc::new(verification::TauriVerification(app.clone()));
    let updates: Arc<dyn tterm_platform::updates::Updates> =
        Arc::new(updates::TauriUpdates(app.clone()));
    let open: Arc<dyn tterm_platform::open::OpenTargets> = Arc::new(open::TauriOpen(app.clone()));
    let clipboard: Arc<dyn tterm_platform::clipboard::Clipboard> =
        Arc::new(clipboard::TauriClipboard(app.clone()));
    let (notifications, integration) = complete(main_window.clone(), badge.clone());
    crate::services::shared::PlatformServices {
        notifications,
        integration,
        badge,
        tray,
        file_drag,
        main_window,
        process,
        verification,
        updates,
        open,
        clipboard,
    }
}

pub(crate) fn main_window_host(app: tauri::AppHandle) -> impl tterm_platform::window::WindowHost {
    window::TauriWindow(app)
}
