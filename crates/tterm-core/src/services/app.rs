use std::sync::Arc;

use crate::launch_dir::LaunchDirs;

use crate::updater::PendingUpdateDownloads;

use crate::background::CloseBehaviorState;
use crate::background::exit_guard::ExitGuardState;
use crate::background::quit::QuitState;
use crate::background::tray::TrayState;
use crate::session_log::SessionLogState;
use crate::tunnel::manager::TunnelManager;

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
