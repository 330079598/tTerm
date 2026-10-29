use super::types::ActivePty;
use portable_pty::{CommandBuilder, PtySize};
use serde::Serialize;
use std::ffi::OsStr;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalShellProfile {
    pub shell: String,
    pub label: String,
    pub source: String,
}

pub fn build_terminal_command(
    shell_config: Option<crate::core::session::types::TerminalShellConfig>,
) -> Result<CommandBuilder, String> {
    let config = shell_config.unwrap_or_default();
    let home = resolve_home_dir();
    let start_dir = resolve_start_dir(config.cwd.as_deref()).unwrap_or_else(|| home.clone());

    #[cfg(target_os = "windows")]
    let mut cmd = super::windows_shell::build_windows_command(&config)?;

    #[cfg(not(target_os = "windows"))]
    let mut cmd = super::unix_shell::build_command(&config, &home)?;

    // After the shell is chosen: $SHELL tells which shell starts.
    #[cfg(unix)]
    super::local_integration::apply(&mut cmd);

    cmd.env("TERM", "xterm-256color");
    // xterm.js supports RGB colors; TUI programs otherwise quantize theme-derived fills.
    cmd.env("COLORTERM", "truecolor");
    cmd.env("HOME", &home);
    if std::env::var_os("LANG").is_none() {
        cmd.env("LANG", "en_US.UTF-8");
    }
    cmd.cwd(&start_dir);
    Ok(cmd)
}

/// The restored directory when it is an absolute path that still exists.
fn resolve_start_dir(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd?;
    // Git Bash reports `/c/dir`, where any Windows shell can start.
    #[cfg(target_os = "windows")]
    let cwd = super::shell_integration::msys_drive_path(cwd).unwrap_or_else(|| cwd.to_string());
    #[cfg(not(target_os = "windows"))]
    let cwd = cwd.to_string();
    let path = std::path::Path::new(&cwd);
    (path.is_absolute() && path.is_dir()).then_some(cwd)
}

#[cfg(target_os = "macos")]
pub(super) fn apply_macos_login_shell_env(cmd: &mut CommandBuilder) {
    for (key, value) in macos_login_shell_env() {
        if should_import_macos_login_shell_env(key) {
            cmd.env(key, value);
        }
    }

    let mut path = macos_shell_path();
    // `tterm` runs in tTerm's tabs whether or not the command is installed.
    // Last, so that it never stands in for a program of the user's.
    if let Some(bin) = crate::system_integration::tab_bin_dir().and_then(std::path::Path::to_str) {
        if !path.split(':').any(|entry| entry == bin) {
            path.push(':');
            path.push_str(bin);
        }
    }
    cmd.env("PATH", path);
}

/// The PATH a local tab starts with, before tTerm's own launcher directory:
/// the login shell's, with the common package-manager locations.
#[cfg(target_os = "macos")]
pub(super) fn macos_shell_path() -> String {
    let inherited = std::env::var_os("PATH");
    let login = macos_login_shell_env()
        .iter()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| OsStr::new(value.as_str()));
    resolve_macos_path(login.or(inherited.as_deref()))
}

/// Loads the login shell's environment in the background, so the first local
/// tab does not wait for `$SHELL -l` (which can take seconds with a heavy
/// profile). A tab opened before it finishes waits for the same load.
#[cfg(target_os = "macos")]
pub fn preload_login_shell_env() {
    std::thread::spawn(|| {
        macos_login_shell_env();
    });
}

#[cfg(target_os = "macos")]
fn macos_login_shell_env() -> &'static [(String, String)] {
    static LOGIN_SHELL_ENV: OnceLock<Vec<(String, String)>> = OnceLock::new();
    LOGIN_SHELL_ENV
        .get_or_init(load_macos_login_shell_env)
        .as_slice()
}

#[cfg(target_os = "macos")]
fn should_import_macos_login_shell_env(key: &str) -> bool {
    !matches!(key, "" | "PWD" | "OLDPWD" | "SHLVL" | "_")
}

#[cfg(target_os = "macos")]
fn load_macos_login_shell_env() -> Vec<(String, String)> {
    let shell = std::env::var("SHELL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string());

    let Ok(output) = std::process::Command::new(shell)
        .args(["-l", "-c", "/usr/bin/env -0"])
        .output()
    else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|entry| entry.split_once('='))
        .filter(|(key, _)| !key.trim().is_empty())
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[cfg(target_os = "macos")]
fn resolve_macos_path(path: Option<&OsStr>) -> String {
    let mut entries: Vec<String> = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.into_os_string().into_string().ok())
        .filter(|entry| !entry.trim().is_empty())
        .collect();

    // GUI-launched .app bundles do not inherit the user's terminal PATH.
    // Seed common macOS package-manager locations before the login shell runs.
    for entry in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        if !entries.iter().any(|existing| existing == entry) {
            entries.push(entry.to_string());
        }
    }

    entries.join(":")
}

/// Finds `executable` in the directories of `path`, a PATH value.
pub(super) fn find_executable_in(executable: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.exists())
}

/// The shells a local tab can start. Async, because finding them on macOS
/// waits for the login shell's PATH.
#[tauri::command]
pub async fn list_available_terminal_shells() -> Result<Vec<TerminalShellProfile>, String> {
    crate::core::blocking::run_blocking(|| {
        #[cfg(target_os = "windows")]
        let shells = super::windows_shell::list_windows_terminal_shells();
        #[cfg(not(target_os = "windows"))]
        let shells = super::unix_shell::list_shells();
        Ok(shells)
    })
    .await
}

/// The program a custom shell's path leads to on this machine, or why it
/// leads nowhere, for the settings to show before a tab tries it.
#[tauri::command]
pub async fn check_local_shell(path: String) -> Result<String, String> {
    crate::core::blocking::run_blocking(move || {
        #[cfg(target_os = "windows")]
        let program = super::windows_shell::resolve_windows_custom_shell(&path);
        #[cfg(not(target_os = "windows"))]
        let program = super::unix_shell::resolve_custom_shell(&path);
        program
    })
    .await
}

/// The name of the shell a local tab with this choice starts (`cmd`, `pwsh`,
/// `zsh`), for the tab's title.
#[tauri::command]
pub fn local_terminal_shell_name(
    shell: Option<String>,
    custom_path: Option<String>,
) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        super::windows_shell::shell_name(shell, custom_path)
    }

    #[cfg(not(target_os = "windows"))]
    {
        if shell.is_some_and(|shell| shell.trim().eq_ignore_ascii_case("custom")) {
            return shell_program_name(&custom_path?);
        }
        shell_program_name(&std::env::var("SHELL").ok()?)
    }
}

pub(super) fn shell_program_name(program: &str) -> Option<String> {
    let name = std::path::Path::new(program.trim()).file_stem()?.to_str()?;
    match name.to_ascii_lowercase().as_str() {
        "" => None,
        "powershell" => Some("PowerShell".to_string()),
        "cmd" | "pwsh" => Some(name.to_ascii_lowercase()),
        _ => Some(name.to_string()),
    }
}

pub(super) fn resolve_home_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let userprofile = userprofile.trim();
            if !userprofile.is_empty() {
                return userprofile.to_string();
            }
        }

        if let (Ok(home_drive), Ok(home_path)) =
            (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH"))
        {
            let home_drive = home_drive.trim();
            let home_path = home_path.trim();
            if !home_drive.is_empty() && !home_path.is_empty() {
                return format!("{}{}", home_drive, home_path);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = home.trim();
            if !home.is_empty() {
                return home.to_string();
            }
        }

        "C:\\".to_string()
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/".to_string())
    }
}

pub fn spawn_local_pty(
    rows: u16,
    cols: u16,
    shell_config: Option<crate::core::session::types::TerminalShellConfig>,
) -> Result<(u32, ActivePty), String> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;
    #[cfg(target_os = "windows")]
    log_conpty_source();

    let cmd = build_terminal_command(shell_config)?;
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let pid = child.process_id().unwrap_or(0);
    drop(pair.slave);

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;

    let active = ActivePty::new(writer, pair.master, child);

    Ok((pid, active))
}

/// Says once which ConPTY the local terminals got. portable-pty falls back to
/// the system one silently when the bundled `conpty.dll` is missing, and the
/// older system ones drop the color queries TUIs send at startup.
#[cfg(target_os = "windows")]
fn log_conpty_source() {
    use windows::core::w;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        // SAFETY: GetModuleHandleW only looks up a module this process already loaded.
        if unsafe { GetModuleHandleW(w!("conpty.dll")) }.is_ok() {
            eprintln!("Local terminals use the bundled ConPTY");
        } else {
            eprintln!(
                "Bundled conpty.dll is missing beside tTerm; local terminals use the system ConPTY"
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{build_terminal_command, shell_program_name};

    #[test]
    fn local_shell_advertises_truecolor() {
        let command = build_terminal_command(None).expect("build local shell");
        assert_eq!(command.get_env("TERM"), Some("xterm-256color".as_ref()));
        assert_eq!(command.get_env("COLORTERM"), Some("truecolor".as_ref()));
    }

    #[test]
    fn names_shells_by_program() {
        assert_eq!(shell_program_name("/bin/zsh").as_deref(), Some("zsh"));
        assert_eq!(shell_program_name("pwsh.exe").as_deref(), Some("pwsh"));
        assert_eq!(shell_program_name("CMD.EXE").as_deref(), Some("cmd"));
        assert_eq!(
            shell_program_name("powershell.exe").as_deref(),
            Some("PowerShell")
        );
        assert_eq!(shell_program_name("  ").as_deref(), None);
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "windows_color_tests.rs"]
mod windows_color_tests;
