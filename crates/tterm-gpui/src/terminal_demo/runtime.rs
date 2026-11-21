//! The GPUI client's production core services and isolated development data.

use std::sync::Arc;

use tokio::sync::mpsc::{self, UnboundedReceiver};
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::identity::AppIdentity;
use tterm_core::paths::AppPaths;
use tterm_core::services::AppServices;
use tterm_core::services::app::AppLevelServices;
use tterm_core::services::session::SessionServices;
use tterm_core::services::sftp::SftpServices;
use tterm_core::services::shared::SharedServices;

mod platform;

/// Keeps the GPUI development data apart from the Tauri application's files.
pub(super) const IDENTIFIER: &str = "com.stone.tTerm.gpui-dev";

/// A numbered debug instance keeps manual acceptance runs out of the developer's
/// database, configuration and OS credential namespace, including across restarts.
#[cfg(debug_assertions)]
fn debug_identifier(instance: Option<std::ffi::OsString>) -> Result<String, String> {
    match instance {
        None => Ok(IDENTIFIER.to_owned()),
        Some(value) => value
            .to_str()
            .and_then(|text| text.parse::<u32>().ok())
            .map(|number| format!("{IDENTIFIER}.check-{number}"))
            .ok_or_else(|| {
                "Set TTERM_GPUI_CHECK_INSTANCE to an unsigned integer or unset it.".into()
            }),
    }
}

/// Services and the runtime that keeps their background work alive.
pub(super) struct GpuiRuntime {
    pub services: Arc<AppServices>,
    pub events: UnboundedReceiver<AppEvent>,
    pub(super) _runtime: tokio::runtime::Runtime,
}

/// Receives events raised by any core service.
pub(super) struct GpuiEventSink {
    sender: mpsc::UnboundedSender<AppEvent>,
}

impl EventSink for GpuiEventSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.sender
            .send(event)
            .map_err(|_| "The GPUI event loop has closed".to_string())
    }
}

/// Initializes the same service graph as Tauri, with GPUI-native adapters.
pub(super) fn start() -> Result<GpuiRuntime, String> {
    tterm_core::identity::install_app_version(env!("CARGO_PKG_VERSION"));
    #[cfg(debug_assertions)]
    let identifier = debug_identifier(std::env::var_os("TTERM_GPUI_CHECK_INSTANCE"))?;
    #[cfg(not(debug_assertions))]
    let identifier = IDENTIFIER.to_owned();
    let paths = Arc::new(runtime_paths(&identifier)?);
    tterm_core::config::paths::init_config_dir(&paths)?;
    tterm_core::db::init()?;

    let identity = Arc::new(AppIdentity {
        identifier,
        name: "tTerm GPUI Dev".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(tterm_core::core::runtime::tokio_worker_threads())
        .enable_all()
        .build()
        .map_err(|error| format!("Failed to start the background runtime: {error}"))?;

    let (sender, event_rx) = mpsc::unbounded_channel();
    let events: Arc<dyn EventSink> = Arc::new(GpuiEventSink { sender });
    let (services, secrets) = build_services(
        Arc::clone(&paths),
        Arc::clone(&identity),
        events,
        runtime.handle().clone(),
    );
    secrets.initialize(identity.as_ref(), &paths.data_dir);

    Ok(GpuiRuntime {
        services,
        events: event_rx,
        _runtime: runtime,
    })
}

fn runtime_paths(identifier: &str) -> Result<AppPaths, String> {
    #[cfg(debug_assertions)]
    if identifier != IDENTIFIER {
        return Ok(isolated_paths(std::env::temp_dir().join(identifier)));
    }
    AppPaths::native(identifier)
}

#[cfg(any(test, debug_assertions))]
fn isolated_paths(root: std::path::PathBuf) -> AppPaths {
    AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        local_data_dir: root.join("local-data"),
        cache_dir: root.join("cache"),
        log_dir: root.join("logs"),
        download_dir: None,
        temp_dir: root.join("temp"),
    }
}

fn build_services(
    paths: Arc<AppPaths>,
    identity: Arc<AppIdentity>,
    events: Arc<dyn EventSink>,
    runtime: tokio::runtime::Handle,
) -> (
    Arc<AppServices>,
    tterm_core::ssh::secret_store::types::SecretStoreState,
) {
    let prompts = Arc::new(tterm_core::prompts::PromptBroker::new(Arc::clone(&events)));
    let secrets = tterm_core::ssh::secret_store::types::SecretStoreState::new();
    tterm_core::ssh::key_vault::install(secrets.clone());

    let services = Arc::new(AppServices {
        shared: SharedServices {
            paths: Arc::clone(&paths),
            identity: Arc::clone(&identity),
            events,
            prompts,
            secrets: secrets.clone(),
            runtime,
            platform: platform::services(identity.as_ref()),
        },
        session: SessionServices {
            ptys: Default::default(),
            zmodem: Default::default(),
            zmodem_armed_send: Default::default(),
            zmodem_manual_detect: Default::default(),
            monitors: Default::default(),
            monitor_prompts: Arc::new(
                tterm_core::monitor::prompts::PendingMonitorPrompts::default(),
            ),
        },
        sftp: SftpServices {
            pool: Default::default(),
            transfer_cancel: Default::default(),
            external_edit: Arc::new(
                tterm_core::sftp::internal::api::external::ExternalEditState::default(),
            ),
        },
        app: AppLevelServices {
            tunnels: Arc::new(tterm_core::tunnel::manager::TunnelManager::default()),
            session_log: Arc::new(tterm_core::session_log::SessionLogState::default()),
            close_behavior: Arc::new(tterm_core::background::CloseBehaviorState::default()),
            quit: Arc::new(tterm_core::background::quit::QuitState::default()),
            exit_guard: Arc::new(tterm_core::background::exit_guard::ExitGuardState::default()),
            launch_dirs: Arc::new(tterm_core::launch_dir::LaunchDirs::default()),
            tray: Arc::new(tterm_core::background::tray::TrayState::default()),
            pending_updates: Arc::new(tterm_core::updater::PendingUpdateDownloads::default()),
        },
    });
    services.validate_wiring();
    (services, secrets)
}

#[cfg(test)]
pub(super) fn test_runtime() -> GpuiRuntime {
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_TEST_DATA: AtomicU32 = AtomicU32::new(1);

    let root = std::env::temp_dir().join(format!(
        "tterm-gpui-test-{}-{}",
        std::process::id(),
        NEXT_TEST_DATA.fetch_add(1, Ordering::Relaxed)
    ));
    let paths = Arc::new(isolated_paths(root));
    let identity = Arc::new(AppIdentity {
        identifier: IDENTIFIER.to_string(),
        name: "tTerm GPUI test".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build GPUI test runtime");
    let (sender, events) = mpsc::unbounded_channel();
    let sink: Arc<dyn EventSink> = Arc::new(GpuiEventSink { sender });
    let (services, _) = build_services(paths, identity, sink, runtime.handle().clone());
    GpuiRuntime {
        services,
        events,
        _runtime: runtime,
    }
}

#[cfg(test)]
mod tests {
    use super::IDENTIFIER;

    #[cfg(debug_assertions)]
    #[test]
    fn acceptance_instances_cannot_select_an_existing_product_identity_or_path() {
        use super::debug_identifier;

        assert_eq!(debug_identifier(None).unwrap(), IDENTIFIER);
        assert_eq!(
            debug_identifier(Some("42".into())).unwrap(),
            format!("{IDENTIFIER}.check-42")
        );
        for invalid in ["", "../tterm", IDENTIFIER, "4294967296"] {
            assert!(debug_identifier(Some(invalid.into())).is_err());
        }
    }

    #[test]
    fn development_data_has_its_own_application_identifier() {
        assert_eq!(IDENTIFIER, "com.stone.tTerm.gpui-dev");
        assert_ne!(IDENTIFIER, "com.stone.tTerm");
    }
}
