//! Thin adapters for Tauri's plugins and window runtime.
mod badge;
mod clipboard;
mod open;
mod tray;
mod updates;
mod verification;
mod window;

use std::sync::Arc;
use tauri::AppHandle;
use tterm_platform::window::WindowHost;

/// Builds the client adapters used by the shared service container.
pub fn initialize(
    app: &AppHandle,
    notifications: Arc<dyn tterm_platform::notifications::Notifications>,
    integration: Result<Arc<dyn tterm_platform::integration::SystemIntegration>, String>,
) -> crate::services::shared::PlatformServices {
    let main_window: Arc<dyn tterm_platform::window::MainWindow> =
        Arc::new(window::TauriWindow(app.clone()));
    let window_host: Arc<dyn WindowHost> = Arc::new(window::TauriWindow(app.clone()));
    crate::services::shared::PlatformServices {
        notifications,
        integration,
        badge: Arc::new(badge::TauriBadge(app.clone())),
        tray: Arc::new(tray::TauriTray::new(app.clone())),
        file_drag: Arc::new(tterm_platform::native::drag::NativeFileDrag(window_host)),
        main_window,
        process: Arc::new(window::TauriProcess(app.clone())),
        verification: Arc::new(verification::TauriVerification(app.clone())),
        updates: Arc::new(updates::TauriUpdates(app.clone())),
        open: Arc::new(open::TauriOpen(app.clone())),
        clipboard: Arc::new(clipboard::TauriClipboard(app.clone())),
    }
}

pub(crate) fn main_window_host(app: tauri::AppHandle) -> impl tterm_platform::window::WindowHost {
    window::TauriWindow(app)
}

/// Creates a window capability without accessing a native window.
pub(crate) fn main_window(app: tauri::AppHandle) -> Arc<dyn tterm_platform::window::MainWindow> {
    Arc::new(window::TauriWindow(app))
}
