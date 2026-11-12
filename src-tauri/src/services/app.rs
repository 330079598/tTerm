use std::sync::Arc;

use crate::{
    background::{
        CloseBehaviorState, exit_guard::ExitGuardState, quit::QuitState, tray::TrayState,
    },
    launch_dir::LaunchDirs,
    session_log::SessionLogState,
    tunnel::commands::TunnelManager,
    updater::PendingUpdateDownloads,
};

pub struct AppLevelServices {
    pub tunnels: Arc<TunnelManager>,
    pub session_log: Arc<SessionLogState>,
    pub close_behavior: Arc<CloseBehaviorState>,
    pub quit: Arc<QuitState>,
    pub exit_guard: Arc<ExitGuardState>,
    pub launch_dirs: Arc<LaunchDirs>,
    pub tray: Arc<TrayState>,
    pub pending_updates: Arc<PendingUpdateDownloads>,
}
