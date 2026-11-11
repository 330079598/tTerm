use super::{app::AppLevelServices, session::SessionServices, sftp::SftpServices};
use crate::{
    background::{
        exit_guard::ExitGuardState, quit::QuitState, tray::TrayState, CloseBehaviorState,
    },
    core::state::{PtyMap, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap},
    launch_dir::LaunchDirs,
    monitor::{prompts::PendingMonitorPrompts, types::MonitorSessionMap},
    session_log::SessionLogState,
    sftp::internal::{
        api::external::ExternalEditState,
        types::{SftpConnectionPool, TransferCancelMap},
    },
    tunnel::commands::TunnelManager,
    updater::PendingUpdateDownloads,
};
use std::sync::Arc;
#[test]
fn app_services_arc_ptr_eq_matches_legacy_states() {
    let ptys: PtyMap = Arc::new(Default::default());
    let zmodem: ZmodemMap = Arc::new(Default::default());
    let zmodem_armed_send: ZmodemArmedSendMap = Arc::new(Default::default());
    let zmodem_manual_detect: ZmodemManualDetectMap = Arc::new(Default::default());
    let monitors: MonitorSessionMap = Arc::new(Default::default());
    let monitor_prompts = Arc::new(PendingMonitorPrompts::default());
    let pool: SftpConnectionPool = Arc::new(Default::default());
    let transfer_cancel: TransferCancelMap = Arc::new(Default::default());
    let external_edit = Arc::new(ExternalEditState::default());
    let tunnels = Arc::new(TunnelManager::default());
    let session_log = Arc::new(SessionLogState::default());
    let close_behavior = Arc::new(CloseBehaviorState::default());
    let quit = Arc::new(QuitState::default());
    let exit_guard = Arc::new(ExitGuardState::default());
    let launch_dirs = Arc::new(LaunchDirs::default());
    let tray = Arc::new(TrayState::default());
    let pending_updates = Arc::new(PendingUpdateDownloads::default());
    let session = SessionServices {
        ptys: ptys.clone(),
        zmodem: zmodem.clone(),
        zmodem_armed_send: zmodem_armed_send.clone(),
        zmodem_manual_detect: zmodem_manual_detect.clone(),
        monitors: monitors.clone(),
        monitor_prompts: monitor_prompts.clone(),
    };
    let sftp = SftpServices {
        pool: pool.clone(),
        transfer_cancel: transfer_cancel.clone(),
        external_edit: external_edit.clone(),
    };
    let app = AppLevelServices {
        tunnels: tunnels.clone(),
        session_log: session_log.clone(),
        close_behavior: close_behavior.clone(),
        quit: quit.clone(),
        exit_guard: exit_guard.clone(),
        launch_dirs: launch_dirs.clone(),
        tray: tray.clone(),
        pending_updates: pending_updates.clone(),
    };
    assert!(Arc::ptr_eq(&session.ptys, &ptys));
    assert!(Arc::ptr_eq(&session.zmodem, &zmodem));
    assert!(Arc::ptr_eq(&session.zmodem_armed_send, &zmodem_armed_send));
    assert!(Arc::ptr_eq(
        &session.zmodem_manual_detect,
        &zmodem_manual_detect
    ));
    assert!(Arc::ptr_eq(&session.monitors, &monitors));
    assert!(Arc::ptr_eq(&session.monitor_prompts, &monitor_prompts));
    assert!(Arc::ptr_eq(&sftp.pool, &pool));
    assert!(Arc::ptr_eq(&sftp.transfer_cancel, &transfer_cancel));
    assert!(Arc::ptr_eq(&sftp.external_edit, &external_edit));
    assert!(Arc::ptr_eq(&app.tunnels, &tunnels));
    assert!(Arc::ptr_eq(&app.session_log, &session_log));
    assert!(Arc::ptr_eq(&app.close_behavior, &close_behavior));
    assert!(Arc::ptr_eq(&app.quit, &quit));
    assert!(Arc::ptr_eq(&app.exit_guard, &exit_guard));
    assert!(Arc::ptr_eq(&app.launch_dirs, &launch_dirs));
    assert!(Arc::ptr_eq(&app.tray, &tray));
    assert!(Arc::ptr_eq(&app.pending_updates, &pending_updates));
}
