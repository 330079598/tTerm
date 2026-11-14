use std::sync::Arc;

use tterm_core::launch_dir::LaunchDirs;

use tterm_core::updater::PendingUpdateDownloads;

use tterm_core::background::CloseBehaviorState;
use tterm_core::background::exit_guard::ExitGuardState;
use tterm_core::background::quit::QuitState;
use tterm_core::background::tray::TrayState;
use tterm_core::session_log::SessionLogState;
use tterm_core::tunnel::manager::TunnelManager;

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
