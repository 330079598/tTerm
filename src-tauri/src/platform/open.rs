use std::{collections::HashMap, path::PathBuf};
use tauri_plugin_opener::OpenerExt;
use tterm_platform::{
    PlatformFuture,
    native::open::launch,
    open::{ApplicationChoice, OpenTargets},
};

pub(super) struct TauriOpen(pub tauri::AppHandle);
impl OpenTargets for TauriOpen {
    fn open_url(&self, url: String) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())
        })
    }
    fn open_file(&self, path: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .opener()
                .open_path(path.to_string_lossy(), None::<&str>)
                .map_err(|e| e.to_string())
        })
    }
    fn reveal(&self, path: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .opener()
                .reveal_item_in_dir(path)
                .map_err(|e| e.to_string())
        })
    }
    fn open_with(&self, program: String, path: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            crate::core::blocking::run_blocking(move || launch::open_with(&program, &path)).await
        })
    }
    fn choose_application(&self, path: PathBuf) -> PlatformFuture<'_, bool> {
        Box::pin(async move {
            crate::core::blocking::run_blocking(move || launch::open_with_system_chooser(&path))
                .await
        })
    }
    fn applications_for(&self, file_name: String) -> PlatformFuture<'_, Vec<ApplicationChoice>> {
        Box::pin(async move {
            crate::core::blocking::run_blocking(move || {
                #[cfg(target_os = "macos")]
                return Ok(tterm_platform::native::open::apps::apps_for(&file_name));
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = file_name;
                    Ok(Vec::new())
                }
            })
            .await
        })
    }
    fn application_icons(
        &self,
        programs: Vec<String>,
    ) -> PlatformFuture<'_, HashMap<String, Option<Vec<u8>>>> {
        Box::pin(async move {
            crate::core::blocking::run_blocking(move || {
                #[cfg(target_os = "macos")]
                return Ok(tterm_platform::native::open::apps::icons_for(programs));
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = programs;
                    Ok(HashMap::new())
                }
            })
            .await
        })
    }
}
