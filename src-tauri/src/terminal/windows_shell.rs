//! The shell a local tab starts on Windows, and the shells offered for it.

use std::path::PathBuf;
use std::sync::OnceLock;

use portable_pty::CommandBuilder;

use super::pty::{find_executable_in, resolve_home_dir, shell_program_name, TerminalShellProfile};
use super::shell_integration::{self, ShellKind};

/// Builds the command a local tab starts, with tTerm's integration for the
/// shells it knows.
pub(super) fn build_windows_command(
    config: &crate::core::session::types::TerminalShellConfig,
) -> Result<CommandBuilder, String> {
    let shell = config.shell.trim().to_ascii_lowercase();

    let found = |path: Option<PathBuf>, name: &str| {
        path.map_or_else(
            || name.to_string(),
            |path| path.to_string_lossy().into_owned(),
        )
    };
    let (program, args): (String, Vec<String>) = match shell.as_str() {
        "cmd" => (found(find_cmd_path(), "cmd.exe"), Vec::new()),
        "powershell" => (
            found(find_windows_powershell_path(), "powershell.exe"),
            vec!["-NoLogo".to_string()],
        ),
        "pwsh" => (
            found(find_pwsh_path(), "pwsh.exe"),
            vec!["-NoLogo".to_string()],
        ),
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

            (resolve_windows_custom_shell(&path)?, args)
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

/// The shell Auto starts: PowerShell 7, then Windows PowerShell, which every
/// supported Windows has, then cmd. It is found once, in the install
/// locations before PATH, so that a PATH entry on an unreachable network
/// share can't hold up every new tab.
fn resolve_windows_auto_shell() -> (String, Vec<String>) {
    static AUTO_SHELL: OnceLock<(String, Vec<String>)> = OnceLock::new();
    AUTO_SHELL
        .get_or_init(|| {
            let powershell = find_pwsh_path().or_else(find_windows_powershell_path);
            if let Some(path) = powershell {
                return (
                    path.to_string_lossy().into_owned(),
                    vec!["-NoLogo".to_string()],
                );
            }
            let cmd = find_cmd_path().map(|path| path.to_string_lossy().into_owned());
            (cmd.unwrap_or_else(|| "cmd.exe".to_string()), Vec::new())
        })
        .clone()
}

/// Finds the program a custom shell names: a path, or a name on PATH whose
/// `.exe` may be left off.
pub(super) fn resolve_windows_custom_shell(shell: &str) -> Result<String, String> {
    let expanded =
        expand_custom_shell_path(shell, |name| std::env::var(name).ok(), &resolve_home_dir());
    let found = if expanded.contains(['\\', '/', ':']) {
        Some(PathBuf::from(&expanded)).filter(|path| path.is_file())
    } else {
        [expanded.clone(), format!("{expanded}.exe")]
            .iter()
            .find_map(|name| find_executable_on_path(name).filter(|path| path.is_file()))
    };
    found
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| {
            format!(
                "The shell {} was not found. Check its path, or choose another shell for this terminal.",
                shell.trim()
            )
        })
}

/// Unquotes a custom shell path, since Explorer's "Copy as path" adds quotes,
/// and expands a leading `~` and `%NAME%` variables. An unknown variable
/// stays as written, as it does in cmd.
fn expand_custom_shell_path(
    shell: &str,
    var: impl Fn(&str) -> Option<String>,
    home: &str,
) -> String {
    let shell = shell.trim();
    let shell = shell
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(shell)
        .trim();
    let shell = match shell.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['\\', '/']) => format!("{home}{rest}"),
        _ => shell.to_string(),
    };

    let mut expanded = String::new();
    let mut rest = shell.as_str();
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            rest = &rest[start..];
            break;
        };
        match var(&after[..end]).filter(|_| end > 0) {
            Some(value) => {
                expanded.push_str(&value);
                rest = &after[end + 1..];
            }
            // The closing `%` may open the next variable.
            None => {
                expanded.push('%');
                rest = after;
            }
        }
    }
    expanded.push_str(rest);
    expanded
}

fn resolve_wsl_command() -> Result<(String, Vec<String>), String> {
    if let Some(path) = find_wsl_path() {
        Ok((path.to_string_lossy().into_owned(), Vec::new()))
    } else {
        Err("WSL is selected but wsl.exe was not found".to_string())
    }
}

fn resolve_git_bash_command() -> Result<(String, Vec<String>), String> {
    if let Some(path) = find_git_bash_path() {
        return Ok((path.to_string_lossy().into_owned(), Vec::new()));
    }

    Err("Git Bash is selected but bash.exe was not found. Install Git for Windows or use Custom executable.".to_string())
}

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

fn is_git_bash_candidate(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }

    path.parent()
        .map(|dir| dir.join("git.exe").exists())
        .unwrap_or(false)
}

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

/// Windows PowerShell where Windows installs it, else on PATH.
fn find_windows_powershell_path() -> Option<PathBuf> {
    env_path("SystemRoot")
        .map(|base| {
            base.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.exists())
        .or_else(|| find_executable_on_path("powershell.exe"))
}

/// PowerShell 7 where its installer puts it, else on PATH (Scoop, winget
/// portable, the Store).
fn find_pwsh_path() -> Option<PathBuf> {
    env_path("ProgramFiles")
        .map(|base| base.join("PowerShell").join("7").join("pwsh.exe"))
        .filter(|path| path.exists())
        .or_else(|| find_executable_on_path("pwsh.exe"))
}

fn find_executable_on_path(executable: &str) -> Option<PathBuf> {
    find_executable_in(executable, std::env::var_os("PATH").as_deref())
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.exists())
}

/// Auto, the shells found on this machine, and an entry to type a path into.
pub(super) fn list_windows_terminal_shells() -> Vec<TerminalShellProfile> {
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

/// The name of the shell a local tab with this choice starts (`cmd`, `pwsh`,
/// `WSL`), for the tab's title.
pub(super) fn shell_name(shell: Option<String>, custom_path: Option<String>) -> Option<String> {
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
        "custom" => expand_custom_shell_path(
            &custom_path?,
            |name| std::env::var(name).ok(),
            &resolve_home_dir(),
        ),
        _ => resolve_windows_auto_shell().0,
    };
    shell_program_name(&program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_custom_shell_paths() {
        let var =
            |name: &str| (name == "LOCALAPPDATA").then(|| r"C:\Users\me\AppData\Local".to_string());
        let expand = |shell: &str| expand_custom_shell_path(shell, var, r"C:\Users\me");
        assert_eq!(
            expand(r#" "C:\Program Files\nu\nu.exe" "#),
            r"C:\Program Files\nu\nu.exe"
        );
        assert_eq!(
            expand(r"%LOCALAPPDATA%\Programs\nu\nu.exe"),
            r"C:\Users\me\AppData\Local\Programs\nu\nu.exe"
        );
        assert_eq!(expand(r"~\bin\nu.exe"), r"C:\Users\me\bin\nu.exe");
        assert_eq!(
            expand(r"%NOPE%\%LOCALAPPDATA%\x"),
            r"%NOPE%\C:\Users\me\AppData\Local\x"
        );
        assert_eq!(expand("50%"), "50%");
        assert_eq!(expand("%%"), "%%");
        assert_eq!(expand("~nu"), "~nu");
    }

    #[test]
    fn finds_windows_shells() {
        let (auto, _) = resolve_windows_auto_shell();
        let auto = auto.to_ascii_lowercase();
        assert!(
            auto.ends_with("pwsh.exe") || auto.ends_with("powershell.exe"),
            "{auto}"
        );

        let cmd = resolve_windows_custom_shell(r#""%SystemRoot%\System32\cmd.exe""#)
            .expect("cmd by path");
        assert!(
            cmd.to_ascii_lowercase().ends_with(r"system32\cmd.exe"),
            "{cmd}"
        );
        let by_name = resolve_windows_custom_shell("cmd").expect("cmd by name");
        assert!(
            by_name.to_ascii_lowercase().ends_with("cmd.exe"),
            "{by_name}"
        );
        assert!(resolve_windows_custom_shell(r"C:\no\such\shell.exe").is_err());
    }
}
