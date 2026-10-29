//! Ties tTerm into the system: the `tterm` command, which opens it from a
//! shell, and "Open in tTerm" in the file manager's menu for a folder.

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod launcher;
#[cfg(target_os = "macos")]
mod macos_command;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "windows")]
mod registry;
#[cfg(target_os = "windows")]
mod user_path;
#[cfg(target_os = "windows")]
mod windows_command;
#[cfg(target_os = "windows")]
mod windows_menu;

#[cfg(target_os = "macos")]
use macos_command as shell_command;
#[cfg(target_os = "macos")]
use macos_menu::FolderMenu;
#[cfg(target_os = "windows")]
use windows_command as shell_command;
#[cfg(target_os = "windows")]
use windows_menu::FolderMenu;

use serde::Serialize;
use tauri::AppHandle;

use crate::config::AppConfig;

/// The folder menu's item in each of tTerm's languages, English first.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const FOLDER_MENU_LABELS: [(&str, &str); 2] = [("en", "Open in tTerm"), ("zh", "在 tTerm 中打开")];

/// The argument tTerm's uninstaller starts it with, to take out what it
/// added to the system while tTerm.exe is still there to do it.
#[cfg(target_os = "windows")]
const REMOVE_ARG: &str = "--remove-system-integration";

/// tTerm's identifier, which names its data directory; the one in
/// tauri.conf.json, which a helper started without Tauri cannot read. A
/// development build has another.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const APP_IDENTIFIER: &str = "com.stone.tTerm";

/// Whether one of tTerm's additions to the system is in place.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    /// Whether tTerm can add it on this system; not on Linux yet.
    supported: bool,
    installed: bool,
}

impl IntegrationStatus {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn supported(installed: bool) -> Self {
        Self {
            supported: true,
            installed,
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    const UNSUPPORTED: Self = Self {
        supported: false,
        installed: false,
    };
}

/// Whether new shells run tTerm as `tterm`.
#[tauri::command]
pub async fn shell_command_status(app: AppHandle) -> Result<IntegrationStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let bin = bin_dir(&app)?;
        crate::core::blocking::run_blocking(move || {
            Ok(IntegrationStatus::supported(shell_command::is_installed(
                &bin,
            )?))
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
        Ok(IntegrationStatus::UNSUPPORTED)
    }
}

/// Adds the `tterm` command or removes it. Where that takes an
/// administrator's password, the system asks for it with `admin_prompt`.
#[tauri::command]
pub async fn set_shell_command(
    app: AppHandle,
    installed: bool,
    admin_prompt: String,
) -> Result<IntegrationStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let bin = bin_dir(&app)?;
        crate::core::blocking::run_blocking(move || {
            let installed = shell_command::set_installed(&bin, installed, &admin_prompt)?;
            Ok(IntegrationStatus::supported(installed))
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, installed, admin_prompt);
        Err("tTerm cannot add the tterm command on this system yet.".to_string())
    }
}

/// Whether the file manager offers "Open in tTerm" for a folder.
#[tauri::command]
pub async fn folder_menu_status(app: AppHandle) -> Result<IntegrationStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let menu = FolderMenu::new(&app)?;
        crate::core::blocking::run_blocking(move || {
            Ok(IntegrationStatus::supported(menu.is_installed()?))
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
        Ok(IntegrationStatus::UNSUPPORTED)
    }
}

/// Adds "Open in tTerm" to the file manager's menu for a folder, named in
/// the app's language, or removes it.
#[tauri::command]
pub async fn set_folder_menu(app: AppHandle, installed: bool) -> Result<IntegrationStatus, String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let menu = FolderMenu::new(&app)?;
        crate::core::blocking::run_blocking(move || {
            let language = crate::config::load_config_file()
                .map(|config| config.language)
                .unwrap_or_default();
            let installed = menu.set_installed(installed, folder_menu_label(&language))?;
            Ok(IntegrationStatus::supported(installed))
        })
        .await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, installed);
        Err("tTerm cannot add a folder menu on this system yet.".to_string())
    }
}

/// Brings what tTerm added to the system up to date with this copy of tTerm,
/// which may have been moved, and with `cfg`, whose language names the
/// folder menu. Called at startup and whenever the settings are saved.
pub fn apply_config(app: &AppHandle, cfg: &AppConfig) {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        if let Err(error) = bin_dir(app).and_then(|bin| shell_command::refresh(&bin)) {
            eprintln!("Failed to update the tterm command: {error}");
        }
        // Every copy of tTerm shares the folder menu, and a development build
        // would take it over each time it starts.
        if app.config().identifier != APP_IDENTIFIER {
            return;
        }
        let label = folder_menu_label(&cfg.language);
        if let Err(error) = FolderMenu::new(app).and_then(|menu| menu.refresh(label)) {
            eprintln!("Failed to update the folder menu: {error}");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = (app, cfg);
}

/// Removes the `tterm` command and the folder menu when tTerm was started by
/// its uninstaller to do so; returns the exit code. Call first thing in
/// `main`, before Tauri starts.
#[cfg(target_os = "windows")]
pub fn run_removal_if_requested() -> Option<i32> {
    if std::env::args_os()
        .nth(1)
        .is_none_or(|arg| arg != REMOVE_ARG)
    {
        return None;
    }
    let mut code = 0;
    let bin = std::env::var_os("LOCALAPPDATA").map(|dir| {
        std::path::PathBuf::from(dir)
            .join(APP_IDENTIFIER)
            .join("bin")
    });
    if let Some(bin) = bin {
        if let Err(error) = shell_command::set_installed(&bin, false, "") {
            eprintln!("Failed to remove the tterm command: {error}");
            code = 1;
        }
    }
    if let Err(error) = windows_menu::FolderMenu.set_installed(false, "") {
        eprintln!("Failed to remove the folder menu: {error}");
        code = 1;
    }
    Some(code)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn folder_menu_label(language: &str) -> &'static str {
    FOLDER_MENU_LABELS
        .iter()
        .find(|(code, _)| *code == language)
        .map_or(FOLDER_MENU_LABELS[0].1, |(_, label)| label)
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

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn the_folder_menu_is_named_in_the_app_language_or_english() {
        assert_eq!(super::folder_menu_label("zh"), "在 tTerm 中打开");
        assert_eq!(super::folder_menu_label("en"), "Open in tTerm");
        assert_eq!(super::folder_menu_label("fr"), "Open in tTerm");
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn the_identifier_is_the_one_tauri_uses() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], super::APP_IDENTIFIER);
    }
}
