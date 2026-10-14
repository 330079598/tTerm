//! Dragging remote files out of tTerm onto a folder of the system file
//! manager. A native drag only carries files that already exist, and remote
//! ones are not downloaded yet, so this module only finds out which folder the
//! drag landed on; the frontend then downloads into it through the usual
//! transfer pipeline, with its conflict prompt, progress and resume.
//!
//! Only Windows and macOS build it; Linux file managers share no protocol
//! for this that tTerm could rely on, so the frontend does not offer it there.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use serde::{Deserialize, Serialize};

/// One dragged entry; the drag shows its name or icon.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DragOutItem {
    pub name: String,
    pub is_dir: bool,
}

/// How a drag out of the window ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DragOutResult {
    /// Dropped on a folder of the file manager or on the desktop.
    Dropped { directory: String },
    /// Let go over no target, over tTerm itself, or cancelled with Escape.
    Cancelled,
    /// Dropped on something that took the drag but is not a plain folder,
    /// such as another app or a zip archive.
    UnknownTarget,
}

/// Starts a native drag of `items` from the window and resolves once it is
/// known where they were dropped. Must be called while the mouse button that
/// started the drag is still held.
#[tauri::command]
pub async fn start_drag_out(
    window: tauri::WebviewWindow,
    items: Vec<DragOutItem>,
) -> Result<DragOutResult, String> {
    if items.is_empty() {
        return Ok(DragOutResult::Cancelled);
    }
    platform_drag_out(window, items).await
}

#[cfg(target_os = "windows")]
async fn platform_drag_out(
    window: tauri::WebviewWindow,
    items: Vec<DragOutItem>,
) -> Result<DragOutResult, String> {
    windows::drag_out(window, items).await
}

#[cfg(target_os = "macos")]
async fn platform_drag_out(
    window: tauri::WebviewWindow,
    items: Vec<DragOutItem>,
) -> Result<DragOutResult, String> {
    macos::drag_out(window, items).await
}
