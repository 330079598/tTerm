use std::sync::Arc;

use tterm_core::launch_dir::LaunchDirs;

use tterm_core::updater::PendingUpdateDownloads;

use crate::{
    background::{
        CloseBehaviorState, exit_guard::ExitGuardState, quit::QuitState, tray::TrayState,
    },
    session_log::SessionLogState,
    tunnel::manager::TunnelManager,
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
