use std::sync::Arc;

use tterm_core::launch_dir::LaunchDirs;

use tterm_core::updater::PendingUpdateDownloads;

use crate::{
    background::CloseBehaviorState, background::exit_guard::ExitGuardState,
    background::quit::QuitState, background::tray::TrayState, tunnel::manager::TunnelManager,
};
use tterm_core::session_log::SessionLogState;

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
