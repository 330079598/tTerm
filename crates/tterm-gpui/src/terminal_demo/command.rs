//! The command the prototype starts.
//!
//! It follows the Tauri version's `build_terminal_command`
//! (`src-tauri/src/terminal/pty.rs`) and its automatic shell choice, so the
//! two clients run the same shell with the same environment and start
//! directory and their output can be compared. Shell integration (prompt
//! marks, working directory reports) is left out; P1-12 replaces this module
//! with the core's session service.

#[cfg(any(windows, test))]
use std::path::PathBuf;

use portable_pty::CommandBuilder;

const USAGE: &str = "usage: tterm-gpui [--ssh <target> [args...] | --exec <program> [args...]]";

/// What the prototype runs in its PTY.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CommandSpec {
    /// The platform's default shell.
    Shell,
    /// A program with arguments: the system `ssh`, the P0 stand-in for
    /// tTerm's own SSH sessions, or a workload a P0 card runs.
    Program { program: String, args: Vec<String> },
}

impl CommandSpec {
    /// Reads the command line after the program name.
    pub fn from_args(arguments: &[String]) -> Result<Self, String> {
        let Some((first, rest)) = arguments.split_first() else {
            return Ok(Self::Shell);
        };
        let (program, args) = match first.as_str() {
            "--ssh" => match rest.first() {
                Some(_) => ("ssh".to_string(), rest.to_vec()),
                None => return Err(format!("--ssh needs a target; {USAGE}")),
            },
            "--exec" => match rest.split_first() {
                Some((program, args)) => (program.clone(), args.to_vec()),
                None => return Err(format!("--exec needs a program; {USAGE}")),
            },
            _ => return Err(USAGE.to_string()),
        };
        Ok(Self::Program { program, args })
    }

    /// The command with the Tauri version's environment and start directory.
    pub fn builder(&self) -> CommandBuilder {
        let home = home_dir(|key| std::env::var(key).ok());
        let mut command = match self {
            Self::Shell => shell_command(),
            Self::Program { program, args } => {
                let mut command = CommandBuilder::new(program);
                command.args(args);
                command
            }
        };
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("HOME", &home);
        if std::env::var_os("LANG").is_none() {
            command.env("LANG", "en_US.UTF-8");
        }
        command.cwd(&home);
        command
    }
}

/// Windows: the Tauri version's automatic shell.
#[cfg(windows)]
fn shell_command() -> CommandBuilder {
    let (program, args) = windows_shell(find_pwsh(), find_windows_powershell(), find_cmd());
    let mut command = CommandBuilder::new(program);
    command.args(args);
    command
}

/// macOS and Linux: the user's login shell, as the Tauri version starts it
/// when no shell is configured.
#[cfg(not(windows))]
fn shell_command() -> CommandBuilder {
    CommandBuilder::new_default_prog()
}

/// PowerShell 7, else Windows PowerShell, both without the banner, else cmd.
#[cfg(any(windows, test))]
pub(super) fn windows_shell(
    pwsh: Option<PathBuf>,
    powershell: Option<PathBuf>,
    cmd: Option<PathBuf>,
) -> (PathBuf, Vec<&'static str>) {
    match pwsh.or(powershell) {
        Some(path) => (path, vec!["-NoLogo"]),
        None => (cmd.unwrap_or_else(|| PathBuf::from("cmd.exe")), Vec::new()),
    }
}

/// PowerShell 7 where its installer puts it, else on PATH.
#[cfg(windows)]
fn find_pwsh() -> Option<PathBuf> {
    env_dir("ProgramFiles")
        .map(|base| base.join("PowerShell").join("7").join("pwsh.exe"))
        .filter(|path| path.exists())
        .or_else(|| find_on_path("pwsh.exe"))
}

/// Windows PowerShell where Windows installs it, else on PATH.
#[cfg(windows)]
fn find_windows_powershell() -> Option<PathBuf> {
    env_dir("SystemRoot")
        .map(|base| {
            base.join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .filter(|path| path.exists())
        .or_else(|| find_on_path("powershell.exe"))
}

/// `COMSPEC`, else cmd in System32, else on PATH.
#[cfg(windows)]
fn find_cmd() -> Option<PathBuf> {
    std::env::var_os("COMSPEC")
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            env_dir("SystemRoot")
                .map(|base| base.join("System32").join("cmd.exe"))
                .filter(|path| path.exists())
        })
        .or_else(|| find_on_path("cmd.exe"))
}

#[cfg(windows)]
fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.exists())
}

#[cfg(windows)]
fn find_on_path(executable: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.exists())
}

/// The home directory as the Tauri version resolves it: on Windows
/// `USERPROFILE`, then `HOMEDRIVE` and `HOMEPATH`, then `HOME`, then `C:\`;
/// elsewhere `HOME`, then `/`. `get` reads one environment variable.
pub(super) fn home_dir(get: impl Fn(&str) -> Option<String>) -> String {
    let value = |key: &str| {
        get(key)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    if cfg!(windows) {
        value("USERPROFILE")
            .or_else(|| Some(format!("{}{}", value("HOMEDRIVE")?, value("HOMEPATH")?)))
            .or_else(|| value("HOME"))
            .unwrap_or_else(|| "C:\\".to_string())
    } else {
        value("HOME").unwrap_or_else(|| "/".to_string())
    }
}
