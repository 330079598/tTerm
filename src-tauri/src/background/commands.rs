//! The Tauri command for resolving a close-window choice from the UI.

use std::sync::Arc;

/// Resolves the UI's close choice using the same close policy as native events.
#[tauri::command]
pub fn resolve_close_request(
    action: String,
    services: tauri::State<'_, Arc<tterm_core::services::AppServices>>,
) {
    tterm_core::background::resolve_close(
        &tterm_core::app_context::background(
            &services,
            tterm_core::sftp::internal::api::external::unuploaded_files(
                &services.sftp.external_edit,
            ),
        ),
        &action,
    );
}
