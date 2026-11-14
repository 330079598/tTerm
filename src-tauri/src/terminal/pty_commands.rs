use tterm_core::{
    terminal::pty::LocalTerminalShellNameInput, terminal::pty::TerminalShellProfile,
    terminal::pty::check_local_shell_service,
    terminal::pty::list_available_terminal_shells_service,
    terminal::pty::local_terminal_shell_name_service,
};

/// The shells a local tab can start. Async, because finding them on macOS
/// waits for the login shell's PATH.
#[tauri::command]
pub async fn list_available_terminal_shells() -> Result<Vec<TerminalShellProfile>, String> {
    list_available_terminal_shells_service().await
}

/// The program a custom shell's path leads to on this machine, or why it
/// leads nowhere, for the settings to show before a tab tries it.
#[tauri::command]
pub async fn check_local_shell(path: String) -> Result<String, String> {
    check_local_shell_service(path).await
}

/// The name of the shell a local tab with this choice starts (`cmd`, `pwsh`,
/// `zsh`), for the tab's title.
#[tauri::command]
pub fn local_terminal_shell_name(
    shell: Option<String>,
    custom_path: Option<String>,
) -> Option<String> {
    local_terminal_shell_name_service(LocalTerminalShellNameInput { shell, custom_path })
}
