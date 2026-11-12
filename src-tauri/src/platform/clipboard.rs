use std::path::PathBuf;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tterm_platform::{PlatformFuture, clipboard::Clipboard};

pub(super) struct TauriClipboard(pub tauri::AppHandle);
impl Clipboard for TauriClipboard {
    fn read_text(&self) -> PlatformFuture<'_, String> {
        Box::pin(async { self.0.clipboard().read_text().map_err(|e| e.to_string()) })
    }
    fn write_text(&self, text: String) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            self.0
                .clipboard()
                .write_text(text)
                .map_err(|e| e.to_string())
        })
    }
    fn read_files(&self) -> PlatformFuture<'_, Vec<PathBuf>> {
        Box::pin(async {
            tterm_core::core::blocking::run_blocking(|| {
                tterm_platform::native::clipboard_files::read_clipboard_file_paths()
                    .map(|paths| paths.into_iter().map(PathBuf::from).collect())
            })
            .await
        })
    }
}
