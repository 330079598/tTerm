//! Thin adapters for Tauri's plugins and window runtime.
mod badge;
mod clipboard;
mod open;
mod tray;
mod updates;
mod verification;
mod window;

use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tterm_platform::{clipboard::Clipboard, open::OpenTargets};

pub fn initialize(app: &AppHandle) {
    app.manage(Arc::new(badge::TauriBadge(app.clone()))
        as Arc<dyn tterm_platform::notifications::AppBadge>);
    app.manage(Arc::new(tray::TauriTray::new(app.clone())) as Arc<dyn tterm_platform::tray::Tray>);
    app.manage(
        Arc::new(tterm_platform::native::drag::NativeFileDrag(Arc::new(
            window::TauriWindow(app.clone()),
        ))) as Arc<dyn tterm_platform::drag::FileDrag>,
    );
    app.manage(
        Arc::new(window::TauriWindow(app.clone())) as Arc<dyn tterm_platform::window::MainWindow>
    );
    app.manage(Arc::new(window::TauriProcess(app.clone()))
        as Arc<dyn tterm_platform::process::ProcessControl>);
    app.manage(Arc::new(verification::TauriVerification(app.clone()))
        as Arc<dyn tterm_platform::verification::UserVerification>);
    app.manage(
        Arc::new(updates::TauriUpdates(app.clone())) as Arc<dyn tterm_platform::updates::Updates>
    );
    app.manage(Arc::new(open::TauriOpen(app.clone())) as Arc<dyn OpenTargets>);
    app.manage(Arc::new(clipboard::TauriClipboard(app.clone())) as Arc<dyn Clipboard>);
}

pub(crate) fn main_window_host(app: tauri::AppHandle) -> impl tterm_platform::window::WindowHost {
    window::TauriWindow(app)
}
