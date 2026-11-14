//! The Tauri command that hands startup directories to the UI.

use std::sync::Arc;

use tterm_core::launch_dir::LaunchDir;

/// Takes the directories waiting to be opened. The frontend calls it once it
/// has restored the session and again on every `launch-dirs`.
#[tauri::command]
pub fn take_launch_dirs(
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) -> Vec<LaunchDir> {
    services.app.launch_dirs.take()
}
