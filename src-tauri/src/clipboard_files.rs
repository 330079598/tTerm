//! Clipboard IPC preserves the existing string path response.
use std::sync::Arc;
use tterm_platform::clipboard::Clipboard;

#[tauri::command]
pub async fn read_clipboard_file_paths(
    clipboard: tauri::State<'_, Arc<dyn Clipboard>>,
) -> Result<Vec<String>, String> {
    clipboard.read_files().await.map(|paths| {
        paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    })
}
