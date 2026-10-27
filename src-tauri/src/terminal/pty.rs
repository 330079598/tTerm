#[cfg(target_os = "windows")]
use super::shell_integration::{self, ShellKind};
use super::types::ActivePty;
use portable_pty::{CommandBuilder, PtySize};
use serde::Serialize;
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
    let mut cmd = build_windows_command(&config)?;

    #[cfg(not(target_os = "windows"))]
    let mut cmd = CommandBuilder::new_default_prog();

    #[cfg(target_os = "macos")]
    apply_macos_login_shell_env(&mut cmd);

    // After the macOS login environment, which tells which shell starts.
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
    let cwd = shell_integration::msys_drive_path(cwd).unwrap_or_else(|| cwd.to_string());
    #[cfg(not(target_os = "windows"))]
    let cwd = cwd.to_string();
    let path = std::path::Path::new(&cwd);
    (path.is_absolute() && path.is_dir()).then_some(cwd)
}

#[cfg(target_os = "macos")]
fn apply_macos_login_shell_env(cmd: &mut CommandBuilder) {
    for (key, value) in macos_login_shell_env() {
        if should_import_macos_login_shell_env(key) {
            cmd.env(key, value);
        }
    }

    cmd.env("PATH", resolve_macos_path(cmd.get_env("PATH")));
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
fn resolve_macos_path(path: Option<&std::ffi::OsStr>) -> String {
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

#[cfg(target_os = "windows")]
fn build_windows_command(
    config: &crate::core::session::types::TerminalShellConfig,
) -> Result<CommandBuilder, String> {
    let shell = config.shell.trim().to_ascii_lowercase();

    let (program, args): (String, Vec<String>) = match shell.as_str() {
        "cmd" => ("cmd.exe".to_string(), Vec::new()),
        "powershell" => ("powershell.exe".to_string(), vec!["-NoLogo".to_string()]),
        "pwsh" => ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]),
        "wsl" => resolve_wsl_command()?,
        "git-bash" => resolve_git_bash_command()?,
        "custom" => {
            let path = config
                .custom_path
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    "Terminal shell is set to custom but no custom path was provided".to_string()
                })?;

            let args = config
                .custom_args
                .as_ref()
                .map(|value| value.split_whitespace().map(|s| s.to_string()).collect())
                .unwrap_or_default();

            (path, args)
        }
        _ => resolve_windows_auto_shell(),
    };

    // Integration that adds arguments stays out of the way of arguments the
    // user chose for a custom shell; environment-only integration is harmless.
    let user_args = shell == "custom" && !args.is_empty();
    let mut args = args;
    let mut env: Vec<(String, String)> = Vec::new();
    let marks = crate::config::command_marks_enabled();

    match ShellKind::from_program(&program) {
        ShellKind::Cmd => {
            let user_prompt = std::env::var("PROMPT").ok();
            env.push((
                "PROMPT".to_string(),
                shell_integration::cmd_prompt(user_prompt.as_deref(), marks),
            ));
        }
        ShellKind::PowerShell if !user_args => {
            args.extend(shell_integration::powershell_args());
            env.extend(shell_integration::powershell_env(marks));
        }
        ShellKind::Bash => {
            let user_prompt_command = std::env::var("PROMPT_COMMAND").ok();
            env.extend(shell_integration::bash_env(
                user_prompt_command.as_deref(),
                marks,
            ));
        }
        ShellKind::Wsl if !user_args => {
            // The WSL shell has to change into a Linux directory itself.
            if let Some(dir) = shell_integration::install() {
                args.extend(shell_integration::wsl_args(
                    &dir,
                    config.cwd.as_deref(),
                    marks,
                ));
            } else {
                // Start in the Linux home, as Windows Terminal does.
                args.extend(["--cd".to_string(), "~".to_string()]);
            }
        }
        _ => {}
    }

    let mut cmd = CommandBuilder::new(&program);
    if !args.is_empty() {
        cmd.args(args);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn resolve_windows_auto_shell() -> (String, Vec<String>) {
    if let Ok(comspec) = std::env::var("COMSPEC") {
        let trimmed = comspec.trim();
        if !trimmed.is_empty() {
            return (trimmed.to_string(), Vec::new());
        }
    }

    if command_exists_on_path("pwsh.exe") {
        return ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]);
    }

    if command_exists_on_path("powershell.exe") {
        return ("powershell.exe".to_string(), vec!["-NoLogo".to_string()]);
    }

    ("cmd.exe".to_string(), Vec::new())
}

#[cfg(target_os = "windows")]
fn resolve_wsl_command() -> Result<(String, Vec<String>), String> {
    if let Some(path) = find_wsl_path() {
        Ok((path.to_string_lossy().into_owned(), Vec::new()))
    } else {
        Err("WSL is selected but wsl.exe was not found".to_string())
    }
}

#[cfg(target_os = "windows")]
fn resolve_git_bash_command() -> Result<(String, Vec<String>), String> {
    if let Some(path) = find_git_bash_path() {
        return Ok((path.to_string_lossy().into_owned(), Vec::new()));
    }

    Err("Git Bash is selected but bash.exe was not found. Install Git for Windows or use Custom executable.".to_string())
}

#[cfg(target_os = "windows")]
fn find_git_bash_path() -> Option<PathBuf> {
    let path_match = find_executable_on_path("bash.exe").filter(|path| is_git_bash_candidate(path));
    if path_match.is_some() {
        return path_match;
    }

    for env_key in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
        if let Some(base) = env_path(env_key) {
            let candidate = base.join("Git").join("bin").join("bash.exe");
            if is_git_bash_candidate(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn is_git_bash_candidate(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }

    path.parent()
        .map(|dir| dir.join("git.exe").exists())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn find_wsl_path() -> Option<PathBuf> {
    if let Some(path) = find_executable_on_path("wsl.exe") {
        return Some(path);
    }

    env_path("SystemRoot")
        .map(|base| base.join("System32").join("wsl.exe"))
        .filter(|path| path.exists())
        .or_else(|| {
            env_path("SystemRoot")
                .map(|base| base.join("Sysnative").join("wsl.exe"))
                .filter(|path| path.exists())
        })
}

#[cfg(target_os = "windows")]
fn find_cmd_path() -> Option<PathBuf> {
    if let Some(comspec) = std::env::var_os("COMSPEC") {
        let path = PathBuf::from(comspec);
        if path.exists() {
            return Some(path);
        }
    }

    env_path("SystemRoot")
        .map(|base| base.join("System32").join("cmd.exe"))
        .filter(|path| path.exists())
        .or_else(|| find_executable_on_path("cmd.exe"))
}

#[cfg(target_os = "windows")]
fn find_windows_powershell_path() -> Option<PathBuf> {
    find_executable_on_path("powershell.exe").or_else(|| {
        env_path("SystemRoot")
            .map(|base| {
                base.join("System32")
                    .join("WindowsPowerShell")
                    .join("v1.0")
                    .join("powershell.exe")
            })
            .filter(|path| path.exists())
    })
}

#[cfg(target_os = "windows")]
fn find_pwsh_path() -> Option<PathBuf> {
    find_executable_on_path("pwsh.exe").or_else(|| {
        env_path("ProgramFiles")
            .map(|base| base.join("PowerShell").join("7").join("pwsh.exe"))
            .filter(|path| path.exists())
    })
}

#[cfg(target_os = "windows")]
fn command_exists_on_path(executable: &str) -> bool {
    find_executable_on_path(executable).is_some()
}

fn find_executable_on_path(executable: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;

    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(executable);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.exists())
}

#[tauri::command]
pub fn list_available_terminal_shells() -> Vec<TerminalShellProfile> {
    #[cfg(target_os = "windows")]
    {
        list_windows_terminal_shells()
    }

    #[cfg(not(target_os = "windows"))]
    {
        list_unix_terminal_shells()
    }
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
        let program = match shell
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "cmd" => "cmd.exe".to_string(),
            "powershell" => "powershell.exe".to_string(),
            "pwsh" => "pwsh.exe".to_string(),
            "wsl" => return Some("WSL".to_string()),
            "git-bash" => return Some("Git Bash".to_string()),
            "custom" => custom_path.filter(|path| !path.trim().is_empty())?,
            _ => resolve_windows_auto_shell().0,
        };
        shell_program_name(&program)
    }

    // Local tabs outside Windows always start the user's default shell.
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (shell, custom_path);
        shell_program_name(&std::env::var("SHELL").ok()?)
    }
}

fn shell_program_name(program: &str) -> Option<String> {
    let name = std::path::Path::new(program.trim()).file_stem()?.to_str()?;
    match name.to_ascii_lowercase().as_str() {
        "" => None,
        "powershell" => Some("PowerShell".to_string()),
        "cmd" | "pwsh" => Some(name.to_ascii_lowercase()),
        _ => Some(name.to_string()),
    }
}

#[cfg(target_os = "windows")]
fn list_windows_terminal_shells() -> Vec<TerminalShellProfile> {
    let mut profiles = vec![TerminalShellProfile {
        shell: "auto".to_string(),
        label: "Auto (recommended)".to_string(),
        source: "tTerm default".to_string(),
    }];

    if let Some(path) = find_cmd_path() {
        profiles.push(TerminalShellProfile {
            shell: "cmd".to_string(),
            label: "Command Prompt (cmd)".to_string(),
            source: path.to_string_lossy().into_owned(),
        });
    }

    if let Some(path) = find_windows_powershell_path() {
        profiles.push(TerminalShellProfile {
            shell: "powershell".to_string(),
            label: "Windows PowerShell".to_string(),
            source: path.to_string_lossy().into_owned(),
        });
    }

    if let Some(path) = find_pwsh_path() {
        profiles.push(TerminalShellProfile {
            shell: "pwsh".to_string(),
            label: "PowerShell 7 (pwsh)".to_string(),
            source: path.to_string_lossy().into_owned(),
        });
    }

    if let Some(path) = find_wsl_path() {
        profiles.push(TerminalShellProfile {
            shell: "wsl".to_string(),
            label: "Windows Subsystem for Linux (WSL)".to_string(),
            source: path.to_string_lossy().into_owned(),
        });
    }

    if let Some(path) = find_git_bash_path() {
        profiles.push(TerminalShellProfile {
            shell: "git-bash".to_string(),
            label: "Git Bash".to_string(),
            source: path.to_string_lossy().into_owned(),
        });
    }

    profiles.push(TerminalShellProfile {
        shell: "custom".to_string(),
        label: "Custom executable".to_string(),
        source: "manual path".to_string(),
    });

    profiles
}

#[cfg(not(target_os = "windows"))]
fn list_unix_terminal_shells() -> Vec<TerminalShellProfile> {
    let mut profiles = vec![TerminalShellProfile {
        shell: "auto".to_string(),
        label: "Auto (recommended)".to_string(),
        source: "tTerm default".to_string(),
    }];

    if let Ok(shell) = std::env::var("SHELL") {
        let shell = shell.trim();
        if !shell.is_empty() {
            let path = PathBuf::from(shell);
            let label = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Default shell");
            profiles.push(TerminalShellProfile {
                shell: "custom".to_string(),
                label: label.to_string(),
                source: shell.to_string(),
            });
        }
    }

    for shell in ["zsh", "fish", "bash", "sh", "dash", "ksh", "elvish", "nu"] {
        if let Some(path) = find_executable_on_path(shell) {
            let source = path.to_string_lossy().into_owned();
            if profiles.iter().any(|profile| profile.source == source) {
                continue;
            }

            profiles.push(TerminalShellProfile {
                shell: "custom".to_string(),
                label: shell.to_string(),
                source,
            });
        }
    }

    profiles
}

fn resolve_home_dir() -> String {
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
