//! The Tauri commands for loading and saving the configuration.

use std::sync::Arc;
use tterm_core::background::CloseBehaviorState;
use tterm_core::background::tray::TrayContext;
use tterm_core::session_log::LogContext;
use tterm_core::{config::AppConfig, config::load_config_file};
use tterm_platform::integration::SystemIntegration;

struct SaveConfigContext {
    log: LogContext,
    close: Arc<CloseBehaviorState>,
    tray: TrayContext,
    integration: Result<Arc<dyn SystemIntegration>, String>,
    runtime: tokio::runtime::Handle,
}

fn apply_saved_config(context: &SaveConfigContext, config: &AppConfig) -> Result<(), String> {
    tterm_core::background::apply_config(&context.close, &context.tray, config);
    context
        .runtime
        .block_on(tterm_core::system_integration::apply_config(
            &context.integration,
            config,
        ));
    context.log.state.apply_config(&context.log, config)
}

#[tauri::command]
pub async fn load_config() -> Result<AppConfig, String> {
    tterm_core::core::blocking::run_blocking(load_config_file).await
}

#[tauri::command]
pub async fn save_config(
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
    config: AppConfig,
) -> Result<(), String> {
    let context = SaveConfigContext {
        log: crate::connection_context::log(&services),
        close: services.app.close_behavior.clone(),
        tray: crate::app_context::tray(&services),
        integration: services.shared.platform.integration.clone(),
        runtime: services.shared.runtime.clone(),
    };
    tterm_core::core::blocking::run_blocking(move || {
        let config = tterm_core::config::save::save_config(context.log.state.clone(), config)?;
        apply_saved_config(&context, &config)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, Weak};
    use std::thread::ThreadId;
    use tterm_core::background::tray::TrayState;
    use tterm_core::events::{AppEvent, EventSink};
    use tterm_core::session_log::SessionLogState;
    use tterm_core::tunnel::manager::TunnelManager;
    use tterm_platform::PlatformFuture;
    use tterm_platform::integration::IntegrationStatus;
    use tterm_platform::tray::{Tray, TrayModel};

    #[derive(Clone, Default)]
    struct Effects {
        calls: Arc<Mutex<Vec<(&'static str, ThreadId)>>>,
    }

    impl Effects {
        fn record(&self, name: &'static str) {
            self.calls
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((name, std::thread::current().id()));
        }
    }

    struct TestTray {
        effects: Effects,
    }

    impl Tray for TestTray {
        fn dispatch(&self, _: Box<dyn FnOnce() + Send>) -> Result<(), String> {
            Ok(())
        }

        fn present(&self, _: TrayModel) -> Result<(), String> {
            Ok(())
        }

        fn remove(&self) -> Result<(), String> {
            self.effects.record("tray");
            Ok(())
        }

        fn is_present(&self) -> bool {
            true
        }

        fn set_tooltip(&self, _: String) -> Result<(), String> {
            Ok(())
        }
    }

    struct TestIntegration {
        effects: Effects,
    }

    impl SystemIntegration for TestIntegration {
        fn shell_status(&self) -> PlatformFuture<'_, IntegrationStatus> {
            Box::pin(async {
                Ok(IntegrationStatus {
                    supported: false,
                    installed: false,
                })
            })
        }

        fn set_shell(&self, _: bool, _: String) -> PlatformFuture<'_, IntegrationStatus> {
            self.shell_status()
        }

        fn folder_status(&self) -> PlatformFuture<'_, IntegrationStatus> {
            self.shell_status()
        }

        fn set_folder(&self, _: bool, _: String) -> PlatformFuture<'_, IntegrationStatus> {
            self.shell_status()
        }

        fn refresh(&self, _: String) -> PlatformFuture<'_, ()> {
            Box::pin(async {
                self.effects.record("integration");
                Ok(())
            })
        }
    }

    struct TestEvents {
        effects: Effects,
    }

    impl EventSink for TestEvents {
        fn emit(&self, _: AppEvent) -> Result<(), String> {
            self.effects.record("session_log");
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn saved_config_effects_run_in_order_on_the_blocking_worker() {
        let effects = Effects::default();
        let native: Arc<dyn Tray> = Arc::new(TestTray {
            effects: effects.clone(),
        });
        let log_state = Arc::new(SessionLogState::default());
        let config = AppConfig {
            close_behavior: "quit".to_string(),
            terminal_log_directory: std::env::temp_dir()
                .join(format!("tterm-p109a2-config-{}", uuid::Uuid::new_v4()))
                .to_string_lossy()
                .into_owned(),
            ..AppConfig::default()
        };
        let context = SaveConfigContext {
            log: LogContext {
                state: log_state,
                events: Arc::new(TestEvents {
                    effects: effects.clone(),
                }),
            },
            close: Arc::new(CloseBehaviorState::default()),
            tray: TrayContext {
                state: Arc::new(TrayState::default()),
                native,
                tunnels: Weak::<TunnelManager>::new(),
            },
            integration: Ok(Arc::new(TestIntegration {
                effects: effects.clone(),
            })),
            runtime: tokio::runtime::Handle::current(),
        };
        let caller = std::thread::current().id();

        tterm_core::core::blocking::run_blocking(move || apply_saved_config(&context, &config))
            .await
            .unwrap();

        let calls = effects
            .calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        assert_eq!(
            calls
                .iter()
                .take(3)
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            ["tray", "integration", "session_log"]
        );
        assert!(calls.iter().all(|(_, thread)| *thread != caller));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn blocking_on_the_current_runtime_from_async_code_panics() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::runtime::Handle::current().block_on(async {});
        }));
        assert!(result.is_err());
    }
}
