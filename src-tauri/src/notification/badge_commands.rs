//! The Tauri command for the unseen-notification badge.

use super::badge::OverlayImage;
use std::sync::Arc;

/// Shows `count` on the app icon, or takes it off at zero. `overlay` is the
/// count drawn for the Windows taskbar; other systems ignore it.
#[tauri::command]
pub async fn set_unseen_notifications(
    count: u32,
    overlay: Option<OverlayImage>,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    super::badge::set_unseen(
        services.shared.platform.main_window.as_ref(),
        services.shared.platform.badge.as_ref(),
        count,
        overlay,
    )
    .await
}
