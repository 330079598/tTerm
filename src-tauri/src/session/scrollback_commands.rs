//! The Tauri commands for saving and restoring terminal scrollback.

use super::scrollback::{SavedScrollback, ScrollbackSnapshot};

#[tauri::command]
pub async fn save_scrollback_snapshots(snapshots: Vec<ScrollbackSnapshot>) -> Result<(), String> {
    super::scrollback::save_scrollback_snapshots_service(snapshots).await
}

/// The saved content of `tab_ids`; the snapshots of other tabs are deleted.
#[tauri::command]
pub async fn load_scrollback_snapshots(
    tab_ids: Vec<String>,
) -> Result<Vec<SavedScrollback>, String> {
    super::scrollback::load_scrollback_snapshots_service(tab_ids).await
}

#[tauri::command]
pub async fn clear_scrollback_snapshots() -> Result<(), String> {
    super::scrollback::clear_scrollback_snapshots_service().await
}
