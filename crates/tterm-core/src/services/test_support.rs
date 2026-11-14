//! Client-neutral service fixtures for integration tests.

use crate::background::CloseBehaviorState;
use crate::background::exit_guard::ExitGuardState;
use crate::background::quit::QuitState;
use crate::background::tray::TrayState;
use crate::events::EventSink;
use crate::events::recording::RecordingEventSink;
use crate::launch_dir::LaunchDirs;
use crate::monitor::prompts::PendingMonitorPrompts;
use crate::services::AppServices;
use crate::services::app::AppLevelServices;
use crate::services::session::SessionServices;
use crate::services::sftp::SftpServices;
use crate::services::shared::{PlatformServices, SharedServices};
use crate::session_log::SessionLogState;
use crate::sftp::internal::api::external::ExternalEditState;
use crate::tunnel::manager::TunnelManager;
use crate::updater::PendingUpdateDownloads;
use std::sync::Arc;
use std::sync::OnceLock;
use tterm_platform::PlatformFuture;
use tterm_platform::clipboard::Clipboard;
use tterm_platform::drag::{DragOutItem, DragOutResult, FileDrag};
use tterm_platform::notifications::{AppBadge, Notifications, RgbaImage};
use tterm_platform::open::{ApplicationChoice, OpenTargets};
use tterm_platform::process::ProcessControl;
use tterm_platform::tray::{Tray, TrayModel};
use tterm_platform::updates::{AvailableUpdate, Updates};
use tterm_platform::verification::UserVerification;
use tterm_platform::window::MainWindow;

/// Services and the runtime that keeps their background tasks alive.
pub(crate) struct TestApp {
    pub services: AppServices,
    pub events: RecordingEventSink,
    pub _runtime: tokio::runtime::Runtime,
}

/// Builds `AppServices` without a Tauri or GPUI application.
pub(crate) fn app() -> TestApp {
    let identifier = "test.p1-12.app-services".to_string();
    let root = std::env::temp_dir().join(format!("tterm-p1-12-{}", std::process::id()));
    let paths = Arc::new(crate::paths::AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        local_data_dir: root.join("local-data"),
        cache_dir: root.join("cache"),
        log_dir: root.join("logs"),
        download_dir: None,
        temp_dir: root.join("temp"),
    });
    static STORAGE: OnceLock<Result<(), String>> = OnceLock::new();
    STORAGE
        .get_or_init(|| {
            crate::config::paths::init_config_dir(&paths)?;
            crate::db::init()?;
            Ok(())
        })
        .as_ref()
        .expect("initialize isolated test storage");
    let identity = Arc::new(crate::identity::AppIdentity {
        identifier: identifier.clone(),
        name: "tTerm P1-12 test".into(),
        version: "0.0.0-test".into(),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build test runtime");
    let events = RecordingEventSink::default();
    let event_sink: Arc<dyn EventSink> = Arc::new(events.clone());
    let notifications: Arc<dyn Notifications> =
        Arc::new(tterm_platform::native::notification::NativeNotifications {
            identifier: identifier.clone(),
            activated: Arc::new(|_| {}),
        });
    let capability = Arc::new(NoopCapability);
    let shared = SharedServices {
        paths,
        identity,
        prompts: Arc::new(crate::prompts::PromptBroker::new(event_sink.clone())),
        events: event_sink,
        secrets: crate::ssh::secret_store::types::SecretStoreState::new(),
        runtime: runtime.handle().clone(),
        platform: PlatformServices {
            notifications,
            integration: Err("not available in tests".into()),
            badge: capability.clone(),
            tray: capability.clone(),
            file_drag: capability.clone(),
            main_window: capability.clone(),
            process: capability.clone(),
            verification: capability.clone(),
            updates: capability.clone(),
            open: capability.clone(),
            clipboard: capability,
        },
    };
    let session = SessionServices {
        ptys: Default::default(),
        zmodem: Default::default(),
        zmodem_armed_send: Default::default(),
        zmodem_manual_detect: Default::default(),
        monitors: Default::default(),
        monitor_prompts: Arc::new(PendingMonitorPrompts::default()),
    };
    let sftp = SftpServices {
        pool: Default::default(),
        transfer_cancel: Default::default(),
        external_edit: Arc::new(ExternalEditState::default()),
    };
    let app = AppLevelServices {
        tunnels: Arc::new(TunnelManager::default()),
        session_log: Arc::new(SessionLogState::default()),
        close_behavior: Arc::new(CloseBehaviorState::default()),
        quit: Arc::new(QuitState::default()),
        exit_guard: Arc::new(ExitGuardState::default()),
        launch_dirs: Arc::new(LaunchDirs::default()),
        tray: Arc::new(TrayState::default()),
        pending_updates: Arc::new(PendingUpdateDownloads::default()),
    };
    TestApp {
        services: AppServices {
            shared,
            session,
            sftp,
            app,
        },
        events,
        _runtime: runtime,
    }
}

struct NoopCapability;

impl AppBadge for NoopCapability {
    fn set_unseen(&self, _: u32, _: Option<RgbaImage>) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn restore(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Tray for NoopCapability {
    fn dispatch(&self, _: Box<dyn FnOnce() + Send>) -> Result<(), String> {
        Ok(())
    }

    fn present(&self, _: TrayModel) -> Result<(), String> {
        Ok(())
    }

    fn remove(&self) -> Result<(), String> {
        Ok(())
    }

    fn is_present(&self) -> bool {
        false
    }

    fn set_tooltip(&self, _: String) -> Result<(), String> {
        Ok(())
    }
}

impl MainWindow for NoopCapability {
    fn save_before_update(&self) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn send_to_background(&self, _: bool) -> Result<(), String> {
        Ok(())
    }

    fn show_and_focus(&self) -> Result<(), String> {
        Ok(())
    }

    fn reveal_for_prompt(&self) -> Result<(), String> {
        Ok(())
    }

    fn set_attention(&self, _: bool) -> Result<(), String> {
        Ok(())
    }
}

impl ProcessControl for NoopCapability {
    fn exit(&self, _: i32) -> Result<(), String> {
        Ok(())
    }

    fn restart(&self) -> Result<(), String> {
        Ok(())
    }
}

impl UserVerification for NoopCapability {
    fn available(&self) -> bool {
        false
    }

    fn verify(&self, _: &str) -> Result<(), String> {
        Err("not available in tests".into())
    }
}

impl Updates for NoopCapability {
    fn check(&self, _: String) -> PlatformFuture<'_, Option<Box<dyn AvailableUpdate>>> {
        Box::pin(async { Ok(None) })
    }
}

impl FileDrag for NoopCapability {
    fn start(&self, _: Vec<DragOutItem>) -> PlatformFuture<'_, DragOutResult> {
        Box::pin(async { Ok(DragOutResult::UnknownTarget) })
    }

    fn refresh_folder(&self, _: std::path::PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl Clipboard for NoopCapability {
    fn read_text(&self) -> PlatformFuture<'_, String> {
        Box::pin(async { Ok(String::new()) })
    }

    fn write_text(&self, _: String) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn read_files(&self) -> PlatformFuture<'_, Vec<std::path::PathBuf>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

impl OpenTargets for NoopCapability {
    fn open_url(&self, _: String) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn open_file(&self, _: std::path::PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn reveal(&self, _: std::path::PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn open_with(&self, _: String, _: std::path::PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn choose_application(&self, _: std::path::PathBuf) -> PlatformFuture<'_, bool> {
        Box::pin(async { Ok(false) })
    }

    fn applications_for(&self, _: String) -> PlatformFuture<'_, Vec<ApplicationChoice>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn application_icons(
        &self,
        _: Vec<String>,
    ) -> PlatformFuture<'_, std::collections::HashMap<String, Option<Vec<u8>>>> {
        Box::pin(async { Ok(std::collections::HashMap::new()) })
    }
}
