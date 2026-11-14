use crate::services::app::AppLevelServices;
use crate::services::session::SessionServices;
use crate::services::sftp::SftpServices;
use crate::updater::PendingUpdateDownloads;

use crate::background::CloseBehaviorState;
use crate::background::exit_guard::ExitGuardState;
use crate::background::quit::QuitState;
use crate::background::tray::TrayState;
use crate::launch_dir::LaunchDirs;
use crate::monitor::prompts::PendingMonitorPrompts;
use crate::session_log::SessionLogState;
use crate::sftp::internal::api::external::ExternalEditState;
use crate::tunnel::manager::TunnelManager;
use crate::{
    core::state::PtyMap, core::state::ZmodemArmedSendMap, core::state::ZmodemManualDetectMap,
    core::state::ZmodemMap, monitor::types::MonitorSessionMap,
    sftp::internal::types::SftpConnectionPool, sftp::internal::types::TransferCancelMap,
};
use std::sync::Arc;

mod session;
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

#[test]
fn added_platform_capabilities_keep_the_same_instances_and_error() {
    let notifications: Arc<dyn tterm_platform::notifications::Notifications> =
        Arc::new(tterm_platform::native::notification::NativeNotifications {
            identifier: "test.p1-07d".into(),
            activated: Arc::new(|_| {}),
        });
    let integration: Arc<dyn tterm_platform::integration::SystemIntegration> =
        Arc::new(tterm_platform::native::integration::IntegrationContext {
            local_data_dir: std::env::temp_dir(),
            identifier: "test.p1-07d".into(),
            home_dir: std::env::temp_dir(),
        });
    let mut platform = platform_with(notifications.clone(), Ok(integration.clone()));
    assert!(Arc::ptr_eq(&platform.notifications, &notifications));
    assert!(Arc::ptr_eq(
        platform.integration.as_ref().unwrap(),
        &integration
    ));
    platform.integration = Err("unavailable".into());
    assert_eq!(platform.integration.err().as_deref(), Some("unavailable"));
}

#[test]
fn shared_paths_and_identity_keep_the_startup_instances() {
    let paths = Arc::new(crate::paths::AppPaths::native("test.p1-07e").unwrap());
    let identity = Arc::new(crate::identity::AppIdentity {
        identifier: "test.p1-07e".into(),
        name: "tTerm".into(),
        version: "0.0.0".into(),
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let events: Arc<dyn crate::events::EventSink> =
        Arc::new(crate::events::recording::RecordingEventSink::default());
    let notifications = Arc::new(tterm_platform::native::notification::NativeNotifications {
        identifier: identity.identifier.clone(),
        activated: Arc::new(|_| {}),
    });
    let shared = crate::services::shared::SharedServices {
        paths: paths.clone(),
        identity: identity.clone(),
        prompts: Arc::new(crate::prompts::PromptBroker::new(events.clone())),
        events,
        secrets: crate::ssh::secret_store::types::SecretStoreState::new(),
        runtime: runtime.handle().clone(),
        platform: platform_with(notifications, Err("unavailable".into())),
    };
    assert!(Arc::ptr_eq(&shared.paths, &paths));
    assert!(Arc::ptr_eq(&shared.identity, &identity));
}

fn platform_with(
    notifications: Arc<dyn tterm_platform::notifications::Notifications>,
    integration: Result<Arc<dyn tterm_platform::integration::SystemIntegration>, String>,
) -> crate::services::shared::PlatformServices {
    let capability = Arc::new(UnusedCapability);
    crate::services::shared::PlatformServices {
        notifications,
        integration,
        badge: capability.clone(),
        tray: capability.clone(),
        file_drag: capability.clone(),
        main_window: capability.clone(),
        process: capability.clone(),
        verification: capability.clone(),
        updates: capability.clone(),
        open: capability.clone(),
        clipboard: capability,
    }
}
struct UnusedCapability;
impl tterm_platform::notifications::AppBadge for UnusedCapability {
    fn set_unseen(
        &self,
        _: u32,
        _: Option<tterm_platform::notifications::RgbaImage>,
    ) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn restore(&self) -> Result<(), String> {
        panic!("unused capability")
    }
}
impl tterm_platform::tray::Tray for UnusedCapability {
    fn dispatch(&self, _: Box<dyn FnOnce() + Send>) -> Result<(), String> {
        panic!("unused capability")
    }
    fn present(&self, _: tterm_platform::tray::TrayModel) -> Result<(), String> {
        panic!("unused capability")
    }
    fn remove(&self) -> Result<(), String> {
        panic!("unused capability")
    }
    fn is_present(&self) -> bool {
        panic!("unused capability")
    }
    fn set_tooltip(&self, _: String) -> Result<(), String> {
        panic!("unused capability")
    }
}
impl tterm_platform::window::MainWindow for UnusedCapability {
    fn save_before_update(&self) -> tterm_platform::PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn send_to_background(&self, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn show_and_focus(&self) -> Result<(), String> {
        panic!("unused capability")
    }
    fn reveal_for_prompt(&self) -> Result<(), String> {
        panic!("unused capability")
    }
    fn set_attention(&self, _: bool) -> Result<(), String> {
        panic!("unused capability")
    }
}
impl tterm_platform::process::ProcessControl for UnusedCapability {
    fn exit(&self, _: i32) -> Result<(), String> {
        panic!("unused capability")
    }
    fn restart(&self) -> Result<(), String> {
        panic!("unused capability")
    }
}
impl tterm_platform::verification::UserVerification for UnusedCapability {
    fn available(&self) -> bool {
        panic!("unused capability")
    }
    fn verify(&self, _: &str) -> Result<(), String> {
        panic!("unused capability")
    }
}
impl tterm_platform::updates::Updates for UnusedCapability {
    fn check(
        &self,
        _: String,
    ) -> tterm_platform::PlatformFuture<'_, Option<Box<dyn tterm_platform::updates::AvailableUpdate>>>
    {
        panic!("unused capability")
    }
}
impl tterm_platform::drag::FileDrag for UnusedCapability {
    fn start(
        &self,
        _: Vec<tterm_platform::drag::DragOutItem>,
    ) -> tterm_platform::PlatformFuture<'_, tterm_platform::drag::DragOutResult> {
        panic!("unused capability")
    }
    fn refresh_folder(&self, _: std::path::PathBuf) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
}
impl tterm_platform::clipboard::Clipboard for UnusedCapability {
    fn read_text(&self) -> tterm_platform::PlatformFuture<'_, String> {
        panic!("unused capability")
    }
    fn write_text(&self, _: String) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn read_files(&self) -> tterm_platform::PlatformFuture<'_, Vec<std::path::PathBuf>> {
        panic!("unused capability")
    }
}
impl tterm_platform::open::OpenTargets for UnusedCapability {
    fn open_url(&self, _: String) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn open_file(&self, _: std::path::PathBuf) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn reveal(&self, _: std::path::PathBuf) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn open_with(
        &self,
        _: String,
        _: std::path::PathBuf,
    ) -> tterm_platform::PlatformFuture<'_, ()> {
        panic!("unused capability")
    }
    fn choose_application(
        &self,
        _: std::path::PathBuf,
    ) -> tterm_platform::PlatformFuture<'_, bool> {
        panic!("unused capability")
    }
    fn applications_for(
        &self,
        _: String,
    ) -> tterm_platform::PlatformFuture<'_, Vec<tterm_platform::open::ApplicationChoice>> {
        panic!("unused capability")
    }
    fn application_icons(
        &self,
        _: Vec<String>,
    ) -> tterm_platform::PlatformFuture<'_, std::collections::HashMap<String, Option<Vec<u8>>>>
    {
        panic!("unused capability")
    }
}
