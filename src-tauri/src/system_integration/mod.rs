//! Ties tTerm into the system: the `tterm` command, which opens it from a shell.

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod launcher;
#[cfg(target_os = "macos")]
mod macos_command;
#[cfg(target_os = "windows")]
mod user_path;
#[cfg(target_os = "windows")]
mod windows_command;

#[cfg(target_os = "macos")]
use macos_command as platform;
#[cfg(target_os = "windows")]
use windows_command as platform;

use serde::Serialize;
use tauri::AppHandle;

/// Whether new shells run tTerm as `tterm`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCommandStatus {
    /// Whether tTerm can add the command on this system; not on Linux yet.
    supported: bool,
    installed: bool,
}

#[tauri::command]
pub async fn shell_command_status(app: AppHandle) -> Result<ShellCommandStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let bin = bin_dir(&app)?;
        crate::core::blocking::run_blocking(move || {
            Ok(ShellCommandStatus {
                supported: true,
                installed: platform::is_installed(&bin)?,
            })
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
        Ok(ShellCommandStatus {
            supported: false,
            installed: false,
        })
    }
}

/// Adds the `tterm` command or removes it. Where that takes an
/// administrator's password, the system asks for it with `admin_prompt`.
#[tauri::command]
pub async fn set_shell_command(
    app: AppHandle,
    installed: bool,
    admin_prompt: String,
) -> Result<ShellCommandStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let bin = bin_dir(&app)?;
        crate::core::blocking::run_blocking(move || {
            Ok(ShellCommandStatus {
                supported: true,
                installed: platform::set_installed(&bin, installed, &admin_prompt)?,
            })
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, installed, admin_prompt);
        Err("tTerm cannot add the tterm command on this system yet.".to_string())
    }
}

/// Keeps an installed `tterm` command pointing at this tTerm, which may have
/// been installed or moved elsewhere since.
pub fn refresh_shell_command(app: &AppHandle) {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    if let Err(error) = bin_dir(app).and_then(|bin| platform::refresh(&bin)) {
        eprintln!("Failed to update the tterm command: {error}");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = app;
}

/// The directory that holds tTerm's `tterm` launchers.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn bin_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("Failed to find tTerm's data directory: {error}"))?
        .join("bin"))
}
