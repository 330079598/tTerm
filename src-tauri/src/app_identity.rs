//! Tauri's startup adapter for the shared application identity.

use tauri::{AppHandle, Manager, Runtime, State};
use tterm_core::identity::AppIdentity;

pub fn initialize(app: &AppHandle) {
    app.manage(AppIdentity {
        identifier: app.config().identifier.clone(),
        name: app.package_info().name.clone(),
        version: app.package_info().version.to_string(),
    });
}

pub fn get<R: Runtime>(app: &AppHandle<R>) -> Result<State<'_, AppIdentity>, String> {
    app.try_state::<AppIdentity>()
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
