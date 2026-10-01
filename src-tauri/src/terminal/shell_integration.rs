//! Shell integration for local terminals on Windows, where a shell's working
//! directory cannot be read from outside (PowerShell's `Set-Location` leaves
//! the process directory alone, WSL's lives in another kernel). Each shell is
//! made to report it at every prompt instead: OSC 9;9 from cmd and
//! PowerShell, OSC 7 from Git Bash and WSL. The frontend stores the reported
//! directory on the tab, so a restored tab starts where the user left off.
//! macOS and Linux read it from the process instead; see `cwd_watch`.

use std::fs;
use std::path::{Path, PathBuf};

/// Exported as a bash function (`BASH_FUNC_<name>%%`) for Git Bash and WSL bash.
const BASH_REPORT_CWD: &str = include_str!("shell_integration/bash/report-cwd.bash");

/// The WSL launcher and the shell scripts it loads, written under
/// `<config dir>/shell-integration` by relative path.
const SCRIPTS: &[(&str, &str)] = &[
    (
        "wsl/launch.sh",
        include_str!("shell_integration/wsl/launch.sh"),
    ),
    ("bash/report-cwd.bash", BASH_REPORT_CWD),
    ("zsh/.zshenv", include_str!("shell_integration/zsh/zshenv")),
    (
        "zsh/.zprofile",
        include_str!("shell_integration/zsh/zprofile"),
    ),
    ("zsh/.zshrc", include_str!("shell_integration/zsh/zshrc")),
    ("zsh/.zlogin", include_str!("shell_integration/zsh/zlogin")),
    (
        "fish/tterm.fish",
        include_str!("shell_integration/fish/tterm.fish"),
    ),
];

const POWERSHELL_PROMPT: &str = include_str!("shell_integration/prompt.ps1");

/// Writes the WSL scripts (only those that changed) and returns their
/// directory, or `None` when they cannot be written; WSL then starts without
/// integration.
pub fn install() -> Option<PathBuf> {
    let dir = crate::config::ensure_config_dir()
        .ok()?
        .join("shell-integration");
    install_into(&dir).ok()?;
    Some(dir)
}

fn install_into(dir: &Path) -> std::io::Result<()> {
    for (relative, content) in SCRIPTS {
        let content = lf(content);
        let path = dir.join(relative);
        if fs::read_to_string(&path).is_ok_and(|existing| existing == content) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
    }
    Ok(())
}

/// A CRLF checkout must not reach the shells as CRLF.
fn lf(content: &str) -> String {
    content.replace("\r\n", "\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Cmd,
    PowerShell,
    Bash,
    Wsl,
    Other,
}

impl ShellKind {
    pub fn from_program(program: &str) -> Self {
        let name = program
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(program)
            .to_ascii_lowercase();
        match name.strip_suffix(".exe").unwrap_or(&name) {
            "cmd" => Self::Cmd,
            "powershell" | "pwsh" => Self::PowerShell,
            "bash" => Self::Bash,
            "wsl" => Self::Wsl,
            _ => Self::Other,
        }
    }
}

/// cmd's `PROMPT` with the working directory report (OSC 9;9) in front.
pub fn cmd_prompt(user_prompt: Option<&str>) -> String {
    let prompt = user_prompt
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("$P$G");
    format!("$E]9;9;$P$E\\{prompt}")
}

/// Runs the prompt wrapper after the profile. Plain text rather than
/// -EncodedCommand, which security software treats as a malware hallmark.
pub fn powershell_args() -> Vec<String> {
    vec![
        "-NoExit".to_string(),
        "-Command".to_string(),
        lf(POWERSHELL_PROMPT),
    ]
}

/// Environment that makes bash report its directory without changing how it
/// starts: an exported function, run from `PROMPT_COMMAND`.
pub fn bash_env(user_prompt_command: Option<&str>) -> Vec<(String, String)> {
    let prompt_command = match user_prompt_command.filter(|value| !value.trim().is_empty()) {
        Some(user) => format!("{user}\n__tterm_report_cwd"),
        None => "__tterm_report_cwd".to_string(),
    };
    vec![
        (
            "BASH_FUNC___tterm_report_cwd%%".to_string(),
            lf(BASH_REPORT_CWD).trim_end().to_string(),
        ),
        ("PROMPT_COMMAND".to_string(), prompt_command),
    ]
}

/// `wsl.exe` arguments that start the user's shell through `wsl/launch.sh`,
/// in `cwd` when it exists there and in `fallback_dir` (a Windows path) otherwise.
pub fn wsl_args(dir: &Path, cwd: Option<&str>, fallback_dir: &str) -> Vec<String> {
    vec![
        "--cd".to_string(),
        dir.to_string_lossy().into_owned(),
        "-e".to_string(),
        "sh".to_string(),
        "./wsl/launch.sh".to_string(),
        cwd.unwrap_or_default().to_string(),
        fallback_dir.to_string(),
    ]
}

/// `C:\dir` for a Git Bash drive path such as `/c/dir`; other MSYS paths
/// (`/usr`, `/tmp`) have no fixed Windows location.
pub fn msys_drive_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix('/')?;
    let mut chars = rest.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    let tail = chars.as_str();
    if !(tail.is_empty() || tail.starts_with('/')) {
        return None;
    }
    Some(format!(
        "{}:\\{}",
        drive.to_ascii_uppercase(),
        tail.trim_start_matches('/').replace('/', "\\")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shells_by_executable_name() {
        assert_eq!(ShellKind::from_program("cmd.exe"), ShellKind::Cmd);
        assert_eq!(
            ShellKind::from_program(r"C:\Windows\System32\CMD.EXE"),
            ShellKind::Cmd
        );
        assert_eq!(
            ShellKind::from_program("powershell.exe"),
            ShellKind::PowerShell
        );
        assert_eq!(ShellKind::from_program("pwsh.exe"), ShellKind::PowerShell);
        assert_eq!(
            ShellKind::from_program(r"C:\Program Files\Git\bin\bash.exe"),
            ShellKind::Bash
        );
        assert_eq!(
            ShellKind::from_program(r"C:\Windows\System32\wsl.exe"),
            ShellKind::Wsl
        );
        assert_eq!(ShellKind::from_program("nu.exe"), ShellKind::Other);
    }

    #[test]
    fn installs_scripts_with_lf_line_endings_and_rewrites_stale_ones() {
        let dir =
            std::env::temp_dir().join(format!("tterm-shell-integration-{}", uuid::Uuid::new_v4()));
        install_into(&dir).unwrap();
        for (relative, _) in SCRIPTS {
            let content = fs::read_to_string(dir.join(relative)).unwrap();
            assert!(!content.contains('\r'), "{relative} has CR line endings");
        }

        let launcher = dir.join("wsl/launch.sh");
        fs::write(&launcher, "stale").unwrap();
        install_into(&dir).unwrap();
        assert!(fs::read_to_string(&launcher).unwrap().contains("wslpath"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cmd_prompt_reports_the_directory_before_the_user_prompt() {
        assert_eq!(cmd_prompt(None), "$E]9;9;$P$E\\$P$G");
        assert_eq!(cmd_prompt(Some("$T $P$G")), "$E]9;9;$P$E\\$T $P$G");
    }

    #[test]
    fn powershell_script_survives_the_windows_command_line() {
        let args = powershell_args();
        assert_eq!(&args[..2], ["-NoExit", "-Command"]);
        assert!(!args[2].contains('"'), "double quotes need escaping");
        assert!(args[2].contains("]9;9;"));
    }

    #[test]
    fn bash_env_exports_the_report_function_after_the_user_prompt_command() {
        let env = bash_env(None);
        assert_eq!(env[0].0, "BASH_FUNC___tterm_report_cwd%%");
        assert!(env[0].1.starts_with("() {") && env[0].1.ends_with('}'));
        assert_eq!(
            env[1],
            ("PROMPT_COMMAND".into(), "__tterm_report_cwd".into())
        );
        assert_eq!(
            bash_env(Some("history -a;"))[1].1,
            "history -a;\n__tterm_report_cwd"
        );
    }

    #[test]
    fn maps_git_bash_drive_paths_to_windows_paths() {
        assert_eq!(
            msys_drive_path("/c/Users/me").as_deref(),
            Some(r"C:\Users\me")
        );
        assert_eq!(msys_drive_path("/d").as_deref(), Some(r"D:\"));
        assert_eq!(msys_drive_path("/d/").as_deref(), Some(r"D:\"));
        assert_eq!(msys_drive_path("/usr/bin"), None);
        assert_eq!(msys_drive_path("/tmp"), None);
        assert_eq!(msys_drive_path(r"C:\Users"), None);
    }
}
