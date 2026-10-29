//! The shell a local tab starts on macOS and Linux, and the shells offered
//! for it.

use std::path::Path;

use super::pty::{find_executable_on_path, TerminalShellProfile};

/// The login shell and the common shells found on PATH.
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
