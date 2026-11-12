use super::foreground::{PtyShellInForegroundInput, pty_shell_in_foreground_service};

/// True when the foreground process group of the tab's local terminal is
/// its shell's own, so no program is running in it. False for SSH sessions,
/// stale sessions and on Windows, where it cannot be told.
#[tauri::command]
pub async fn pty_shell_in_foreground(
    tab_id: String,
    session_nonce: u32,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let state = services.session.ptys.clone();
    pty_shell_in_foreground_service(
        state,
        PtyShellInForegroundInput {
            tab_id,
            session_nonce,
        },
    )
    .await
}
