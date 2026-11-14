//! Clipboard IPC preserves the existing string path response.
use std::sync::Arc;
use tterm_core::services::AppServices;

#[tauri::command]
pub async fn read_clipboard_file_paths(
    services: tauri::State<'_, Arc<AppServices>>,
) -> Result<Vec<String>, String> {
    services
        .shared
        .platform
        .clipboard
        .read_files()
        .await
        .map(|paths| {
            paths
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect()
        })
}
