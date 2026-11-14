//! Tauri's startup adapter for the shared application identity.

use std::sync::Arc;
use tauri::AppHandle;
#[cfg(any(all(unix, not(target_os = "macos")), test))]
use tauri::Manager;
#[cfg(any(all(unix, not(target_os = "macos")), test))]
use tauri::Runtime;
use tterm_core::identity::AppIdentity;
#[cfg(any(all(unix, not(target_os = "macos")), test))]
use tterm_core::services::AppServices;

/// Captures Tauri identity at startup without changing storage or keyring naming.
pub fn initialize(app: &AppHandle) -> Arc<AppIdentity> {
    Arc::new(AppIdentity {
        identifier: app.config().identifier.clone(),
        name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
    })
}

/// Returns the startup identity or an actionable error before initialization.
#[cfg(any(all(unix, not(target_os = "macos")), test))]
pub fn get<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<AppIdentity>, String> {
    app.try_state::<Arc<AppServices>>()
        .map(|services| services.shared.identity.clone())
        .ok_or_else(|| "Application identity is not ready yet".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn uninitialized_identity_is_an_error() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        assert!(super::get(app.handle()).is_err());
    }
}
