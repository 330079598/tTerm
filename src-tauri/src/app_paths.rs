//! Tauri's startup adapter for the paths consumed by domain services.

use crate::services::AppServices;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tterm_core::paths::AppPaths;

/// Resolves paths with Tauri, including any application directory overrides.
/// The identifier must match the handle, whose resolver owns that identity.
pub(crate) fn from_tauri<R: Runtime>(
    app: &AppHandle<R>,
    identifier: &str,
) -> Result<AppPaths, String> {
    if app.config().identifier != identifier {
        return Err("Application path identifier does not match the Tauri configuration".into());
    }
    let path = app.path();
    let error = |error| format!("Failed to resolve application directory: {error}");
    Ok(AppPaths {
        config_dir: path.app_config_dir().map_err(error)?,
        data_dir: path.app_data_dir().map_err(error)?,
        local_data_dir: path.app_local_data_dir().map_err(error)?,
        cache_dir: path.app_cache_dir().map_err(error)?,
        log_dir: path.app_log_dir().map_err(error)?,
        download_dir: path.download_dir().ok(),
        temp_dir: path.temp_dir().map_err(error)?,
    })
}

/// Initializes the config directory and returns paths for the shared container.
pub(crate) fn initialize(app: &AppHandle) -> Result<Arc<AppPaths>, String> {
    let paths = from_tauri(app, &app.config().identifier)?;
    tterm_core::config::paths::init_config_dir(&paths)?;
    Ok(Arc::new(paths))
}

/// Reports unavailable startup state without panicking in early callbacks.
pub(crate) fn get<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<AppPaths>, String> {
    app.try_state::<Arc<AppServices>>()
        .map(|services| services.shared.paths.clone())
        .ok_or_else(|| "Application directories are not ready yet".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn paths_before_initialization_return_an_error() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("build mock app without initializing paths");
        let error = get(app.handle()).expect_err("paths must not be registered yet");
        assert_eq!(error, "Application directories are not ready yet");
    }

    #[test]
    fn native_and_tauri_resolve_identical_paths_for_each_identity() {
        for identifier in [
            "com.stone.tTerm",
            "com.stone.tTerm.dev",
            "com.stone.tTerm.gpui-dev",
        ] {
            let mut context = mock_context(noop_assets());
            context.config_mut().identifier = identifier.into();
            let app = mock_builder().build(context).expect("build mock app");
            let tauri = from_tauri(app.handle(), identifier).expect("resolve Tauri paths");
            let native = AppPaths::native(identifier).expect("resolve native paths");
            assert_eq!(native, tauri, "identity: {identifier}");
            assert!(tauri.config_dir.ends_with(identifier));
            assert!(tauri.data_dir.ends_with(identifier));
            assert!(tauri.local_data_dir.ends_with(identifier));
            assert!(tauri.cache_dir.ends_with(identifier));
            #[cfg(target_os = "macos")]
            assert!(
                tauri
                    .log_dir
                    .ends_with(format!("Library/Logs/{identifier}"))
            );
            #[cfg(not(target_os = "macos"))]
            assert_eq!(tauri.log_dir, tauri.local_data_dir.join("logs"));
            assert!(from_tauri(app.handle(), "different.identity").is_err());
        }
    }
    #[test]
    fn tauri_directory_override_remains_authoritative() {
        use tauri::utils::config::AppDirectoriesOverride;
        let mut context = mock_context(noop_assets());
        context.config_mut().identifier = "com.stone.tTerm.gpui-dev".into();
        let root = std::env::temp_dir().join("tterm-path-override");
        context.config_mut().app.app_directories_override =
            Some(AppDirectoriesOverride::Root(root.clone()));
        let app = mock_builder().build(context).expect("build mock app");
        let paths = from_tauri(app.handle(), &app.config().identifier).unwrap();
        assert_eq!(paths.config_dir, root);
        assert_eq!(paths.data_dir, root);
        assert_eq!(paths.local_data_dir, root);
        assert_eq!(paths.cache_dir, root.join("caches"));
        assert_eq!(paths.log_dir, root.join("logs"));
    }
}
