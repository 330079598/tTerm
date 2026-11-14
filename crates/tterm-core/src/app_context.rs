//! Entry adapters assemble concrete dependencies before handing work to services.
use crate::background::BackgroundContext;
use crate::background::quit::QuitContext;
use crate::background::tray::TrayContext;
use crate::services::AppServices;
use crate::tunnel::manager::TunnelContext;
use std::sync::Arc;

pub fn tray(services: &AppServices) -> TrayContext {
    TrayContext {
        state: services.app.tray.clone(),
        native: services.shared.platform.tray.clone(),
        tunnels: Arc::downgrade(&services.app.tunnels),
    }
}

pub fn quit(services: &AppServices) -> QuitContext {
    QuitContext {
        state: services.app.quit.clone(),
        events: services.shared.events.clone(),
        process: services.shared.platform.process.clone(),
        runtime: services.shared.runtime.clone(),
    }
}

pub fn background(services: &AppServices, unuploaded_files: Vec<String>) -> BackgroundContext {
    BackgroundContext {
        close: services.app.close_behavior.clone(),
        guard: services.app.exit_guard.clone(),
        tunnels: services.app.tunnels.clone(),
        unuploaded_files,
        window: services.shared.platform.main_window.clone(),
        badge: services.shared.platform.badge.clone(),
        events: services.shared.events.clone(),
        quit: quit(services),
        tray: tray(services),
    }
}

pub fn tunnel(services: &AppServices) -> TunnelContext {
    TunnelContext {
        connection: crate::connection_context::connection(services),
        secrets: services.shared.secrets.clone(),
        tray: services.shared.platform.tray.clone(),
        tray_state: services.app.tray.clone(),
        manager: Arc::downgrade(&services.app.tunnels),
    }
}
