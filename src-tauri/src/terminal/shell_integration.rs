//! Shell integration for local terminals on Windows, where a shell's working
//! directory cannot be read from outside (PowerShell's `Set-Location` leaves
//! the process directory alone, WSL's lives in another kernel). Each shell is
//! made to report it at every prompt instead: OSC 9;9 from cmd and
//! PowerShell, OSC 7 from Git Bash and WSL. The frontend stores the reported
//! directory on the tab, so a restored tab starts where the user left off.
//! macOS and Linux read it from the process instead; see `cwd_watch`.
//!
//! Unless turned off in the settings, the shells also mark their commands
//! (OSC 133): A where a prompt starts, C where a command's output starts, D
//! with its exit status once it ends. cmd sends only A, PowerShell A and D.

use std::fs;
use std::path::{Path, PathBuf};

/// Exported as a bash function (`BASH_FUNC_<name>%%`) for Git Bash and WSL bash.
const BASH_REPORT_CWD: &str = include_str!("shell_integration/bash/report-cwd.bash");
/// The directory report plus command marks (OSC 133 D and A).
const BASH_PROMPT: &str = include_str!("shell_integration/bash/prompt.bash");
/// bash prints `PS0` after reading a command and before running it.
const BASH_PS0: &str = r"\e]133;C\e\\";

/// Makes the PowerShell, zsh and fish scripts mark commands.
const COMMAND_MARKS_ENV: &str = "TTERM_COMMAND_MARKS";

/// The WSL launcher and the shell scripts it loads, written under
/// `<config dir>/shell-integration` by relative path.
const SCRIPTS: &[(&str, &str)] = &[
    (
        "wsl/launch.sh",
        include_str!("shell_integration/wsl/launch.sh"),
    ),
    ("bash/report-cwd.bash", BASH_REPORT_CWD),
    ("bash/prompt.bash", BASH_PROMPT),
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

/// cmd's `PROMPT` with the working directory report (OSC 9;9) and, with
/// `marks`, the prompt start mark (OSC 133;A) in front.
pub fn cmd_prompt(user_prompt: Option<&str>, marks: bool) -> String {
    let prompt = user_prompt
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("$P$G");
    let mark = if marks { "$E]133;A$E\\" } else { "" };
    format!("$E]9;9;$P$E\\{mark}{prompt}")
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

/// Environment that makes PowerShell mark its commands.
pub fn powershell_env(marks: bool) -> Vec<(String, String)> {
    if marks {
        vec![(COMMAND_MARKS_ENV.to_string(), "1".to_string())]
    } else {
        Vec::new()
    }
}

/// Environment that makes bash report its directory without changing how it
/// starts: an exported function, run from `PROMPT_COMMAND`. With `marks` the
/// function marks the prompt as well and runs first, while `$?` is still the
/// command's, and `PS0` marks where each command's output starts.
pub fn bash_env(user_prompt_command: Option<&str>, marks: bool) -> Vec<(String, String)> {
    let user = user_prompt_command.filter(|value| !value.trim().is_empty());
    if marks {
        let prompt_command = match user {
            Some(user) => format!("__tterm_prompt\n{user}"),
            None => "__tterm_prompt".to_string(),
        };
        return vec![
            (
                "BASH_FUNC___tterm_prompt%%".to_string(),
                lf(BASH_PROMPT).trim_end().to_string(),
            ),
            ("PROMPT_COMMAND".to_string(), prompt_command),
            ("PS0".to_string(), BASH_PS0.to_string()),
        ];
    }
    let prompt_command = match user {
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
/// in `cwd` when it exists there and in `fallback_dir` (a Windows path)
/// otherwise; with `marks` the shell marks its commands.
pub fn wsl_args(dir: &Path, cwd: Option<&str>, fallback_dir: &str, marks: bool) -> Vec<String> {
    vec![
        "--cd".to_string(),
        dir.to_string_lossy().into_owned(),
        "-e".to_string(),
        "sh".to_string(),
        "./wsl/launch.sh".to_string(),
        cwd.unwrap_or_default().to_string(),
        fallback_dir.to_string(),
        if marks { "1" } else { "0" }.to_string(),
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
        assert_eq!(cmd_prompt(None, false), "$E]9;9;$P$E\\$P$G");
        assert_eq!(cmd_prompt(Some("$T $P$G"), false), "$E]9;9;$P$E\\$T $P$G");
        assert_eq!(cmd_prompt(None, true), "$E]9;9;$P$E\\$E]133;A$E\\$P$G");
    }

    #[test]
    fn powershell_script_survives_the_windows_command_line() {
        let args = powershell_args();
        assert_eq!(&args[..2], ["-NoExit", "-Command"]);
        assert!(!args[2].contains('"'), "double quotes need escaping");
        assert!(args[2].contains("]9;9;"));
        assert!(args[2].contains(COMMAND_MARKS_ENV));
        assert!(powershell_env(false).is_empty());
        assert_eq!(powershell_env(true)[0].0, COMMAND_MARKS_ENV);
    }

    #[test]
    fn bash_env_exports_the_report_function_after_the_user_prompt_command() {
        let env = bash_env(None, false);
        assert_eq!(env[0].0, "BASH_FUNC___tterm_report_cwd%%");
        assert!(env[0].1.starts_with("() {") && env[0].1.ends_with('}'));
        assert_eq!(
            env[1],
            ("PROMPT_COMMAND".into(), "__tterm_report_cwd".into())
        );
        assert_eq!(
            bash_env(Some("history -a;"), false)[1].1,
            "history -a;\n__tterm_report_cwd"
        );
    }

    #[test]
    fn bash_env_with_marks_runs_the_prompt_function_before_the_user_prompt_command() {
        let env = bash_env(Some("history -a;"), true);
        assert_eq!(env[0].0, "BASH_FUNC___tterm_prompt%%");
        assert!(env[0].1.starts_with("() {") && env[0].1.ends_with('}'));
        assert!(env[0].1.contains("]133;D;") && env[0].1.contains("]133;A"));
        assert_eq!(
            env[1],
            (
                "PROMPT_COMMAND".into(),
                "__tterm_prompt\nhistory -a;".into()
            )
        );
        assert_eq!(env[2], ("PS0".into(), r"\e]133;C\e\\".into()));
    }

    #[test]
    fn wsl_args_end_with_the_marks_switch() {
        let args = wsl_args(Path::new("dir"), None, r"C:\Users\me", true);
        assert_eq!(args[args.len() - 3..], ["", r"C:\Users\me", "1"]);
        let args = wsl_args(Path::new("dir"), Some("/home/me"), r"C:\Users\me", false);
        assert_eq!(args.last().map(String::as_str), Some("0"));
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
