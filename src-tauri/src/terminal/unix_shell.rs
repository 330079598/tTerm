//! The shell a local tab starts on macOS and Linux, and the shells offered
//! for it.

use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

use super::pty::{find_executable_in, TerminalShellProfile};
use crate::core::session::types::TerminalShellConfig;

/// Builds the command a local tab starts. `auto`, and the Windows-only
/// choices a profile synced from Windows may carry, start the login shell;
/// `custom` starts the chosen program.
pub(super) fn build_command(
    config: &TerminalShellConfig,
    home: &str,
) -> Result<CommandBuilder, String> {
    let custom = if config.shell.trim().eq_ignore_ascii_case("custom") {
        let path = config
            .custom_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .ok_or_else(|| {
                "Terminal shell is set to custom but no custom path was provided".to_string()
            })?;
        Some(resolve_shell(path, search_path().as_deref(), home)?)
    } else {
        None
    };
    let args: Vec<&str> = config
        .custom_args
        .as_deref()
        .map(|args| args.split_whitespace().collect())
        .unwrap_or_default();

    let mut cmd = match &custom {
        Some(program) if !args.is_empty() => {
            let mut cmd = CommandBuilder::new(program);
            cmd.args(args);
            cmd
        }
        _ => CommandBuilder::new_default_prog(),
    };

    #[cfg(target_os = "macos")]
    super::pty::apply_macos_login_shell_env(&mut cmd);

    // Without arguments the chosen shell starts the way the login shell does,
    // as `-fish`, and becomes $SHELL: tTerm's integration reads it to tell
    // which shell starts, and so do programs that start one (tmux, vim's :sh).
    if let Some(program) = custom.filter(|_| cmd.is_default_prog()) {
        cmd.env("SHELL", program);
    }
    Ok(cmd)
}

/// Finds the program a custom shell names: a path, a path under `~/`, or a
/// name to look up on PATH.
fn resolve_shell(shell: &str, search_path: Option<&OsStr>, home: &str) -> Result<PathBuf, String> {
    let found = if let Some(rest) = shell.strip_prefix("~/") {
        Some(Path::new(home).join(rest))
    } else if shell.contains('/') {
        Some(PathBuf::from(shell))
    } else {
        find_executable_in(shell, search_path)
    };
    found.filter(|path| is_executable(path)).ok_or_else(|| {
        format!(
            "The shell {shell} was not found or can't be run. Check its path, or choose another shell for this terminal."
        )
    })
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The PATH a local tab gets. An app opened from the Finder doesn't inherit
/// the user's, so on macOS it is the login shell's.
fn search_path() -> Option<OsString> {
    #[cfg(target_os = "macos")]
    {
        Some(super::pty::macos_shell_path().into())
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("PATH")
    }
}

/// The login shell, the common shells found on PATH, and an entry to type
/// a path into.
pub(super) fn list_shells() -> Vec<TerminalShellProfile> {
    let mut profiles = vec![TerminalShellProfile {
        shell: "auto".to_string(),
        label: "Auto (recommended)".to_string(),
        source: "tTerm default".to_string(),
    }];

    if let Ok(shell) = std::env::var("SHELL") {
        let shell = shell.trim();
        if !shell.is_empty() {
            let label = Path::new(shell)
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

    let search_path = search_path();
    for shell in ["zsh", "fish", "bash", "sh", "dash", "ksh", "elvish", "nu"] {
        if let Some(path) = find_executable_in(shell, search_path.as_deref()) {
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

    profiles.push(TerminalShellProfile {
        shell: "custom".to_string(),
        label: "Custom executable".to_string(),
        source: "manual path".to_string(),
    });

    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(path: &str, args: Option<&str>) -> TerminalShellConfig {
        TerminalShellConfig {
            shell: "custom".to_string(),
            custom_path: Some(path.to_string()),
            custom_args: args.map(str::to_string),
            cwd: None,
        }
    }

    #[test]
    fn finds_shells_by_path_home_or_name() {
        let search = OsStr::new("/nonexistent:/bin");
        assert_eq!(
            resolve_shell("/bin/sh", None, "/home/me"),
            Ok(PathBuf::from("/bin/sh"))
        );
        assert_eq!(
            resolve_shell("sh", Some(search), "/home/me"),
            Ok(PathBuf::from("/bin/sh"))
        );
        assert_eq!(
            resolve_shell("~/sh", None, "/bin"),
            Ok(PathBuf::from("/bin/sh"))
        );
        assert!(resolve_shell("no-such-shell", Some(search), "/").is_err());
        assert!(resolve_shell("/etc/hosts", None, "/").is_err());
    }

    #[test]
    fn starts_a_chosen_shell_as_the_login_shell() {
        let cmd = build_command(&custom("/bin/sh", None), "/").expect("build");
        assert!(cmd.is_default_prog());
        assert_eq!(cmd.get_env("SHELL"), Some(OsStr::new("/bin/sh")));
    }

    #[test]
    fn starts_a_chosen_shell_with_its_arguments_as_given() {
        let cmd = build_command(&custom("/bin/sh", Some("-i  -x")), "/").expect("build");
        assert_eq!(cmd.get_argv(), &["/bin/sh", "-i", "-x"]);
    }

    #[test]
    fn starts_the_login_shell_for_other_choices() {
        let config = TerminalShellConfig {
            shell: "pwsh".to_string(),
            ..TerminalShellConfig::default()
        };
        assert!(build_command(&config, "/")
            .expect("build")
            .is_default_prog());
        assert!(build_command(&custom(" ", None), "/").is_err());
    }
}
