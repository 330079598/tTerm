//! Opt-in shell integration for local terminals on macOS and Linux. The
//! backend already reads a local shell's directory from its process
//! (`cwd_watch`), so this is only for command marks (OSC 133): when turned on
//! in the settings, bash, zsh and fish start through the same `unix/start.sh`
//! as SSH sessions instead of directly. Other shells start as before.

use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

use super::shell_scripts::{write_scripts, UNIX_SCRIPTS};

/// The shells `unix/start.sh` integrates.
fn is_supported(shell: &str) -> bool {
    matches!(
        Path::new(shell).file_name().and_then(|name| name.to_str()),
        Some("bash" | "zsh" | "fish")
    )
}

/// Writes the scripts (only those that changed) under
/// `<config dir>/shell-integration` and returns that directory, or `None`
/// when they cannot be written; the shell then starts without integration.
fn install() -> Option<PathBuf> {
    let dir = crate::config::ensure_config_dir()
        .ok()?
        .join("shell-integration");
    write_scripts(&dir, UNIX_SCRIPTS.iter().copied()).ok()?;
    Some(dir)
}

/// Turns `cmd`, which starts the user's login shell, into one that starts it
/// through `<dir>/unix/start.sh`, keeping its environment and directory.
fn launch_through(cmd: &mut CommandBuilder, dir: &Path) {
    cmd.get_argv_mut()
        .extend(["/bin/sh".into(), dir.join("unix/start.sh").into_os_string()]);
}

/// Starts the login shell `cmd` through the integration when it is turned
/// on, command marks are on and the shell is bash, zsh or fish.
pub fn apply(cmd: &mut CommandBuilder) {
    if !cmd.is_default_prog()
        || !crate::config::local_shell_integration_enabled()
        || !crate::config::command_marks_enabled()
        || !is_supported(&cmd.get_shell())
    {
        return;
    }
    if let Some(dir) = install() {
        launch_through(cmd, &dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_bash_zsh_and_fish_only() {
        assert!(is_supported("/bin/zsh"));
        assert!(is_supported("/opt/homebrew/bin/bash"));
        assert!(is_supported("/usr/bin/fish"));
        assert!(!is_supported("/bin/sh"));
        assert!(!is_supported("/usr/local/bin/nu"));
        assert!(!is_supported("/usr/bin/zsh-5.9"));
    }

    #[test]
    fn launches_the_login_shell_through_start_sh_with_its_environment() {
        let mut cmd = CommandBuilder::new_default_prog();
        cmd.env("TERM", "xterm-256color");
        launch_through(&mut cmd, Path::new("/config/shell-integration"));
        assert!(!cmd.is_default_prog());
        assert_eq!(
            cmd.get_argv(),
            &["/bin/sh", "/config/shell-integration/unix/start.sh"]
        );
        assert_eq!(cmd.get_env("TERM"), Some("xterm-256color".as_ref()));
    }
}
