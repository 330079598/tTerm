//! The label a detected shell shows, from `src/lib/terminalShells.ts`.

use crate::terminal::pty::TerminalShellProfile;
use crate::workspace::tab::TerminalShellType;

use super::shell_type_from_id;

/// The catalogue key the frontend builds for an id its translation table has
/// no entry for: the template's `${...}` interpolates `undefined`.
const UNKNOWN_SHELL_LABEL_KEY: &str = "connection.terminalShellOptions.undefined";

/// Whether a `custom` profile is a shell found on this machine rather than an
/// entry to type a path into.
pub fn is_detected_custom_shell(profile: &TerminalShellProfile) -> bool {
    profile.shell == "custom" && profile.source != "manual path" && profile.source != "$SHELL"
}

/// The `connection.terminalShellOptions` catalogue key a shell kind's label is
/// translated from.
pub fn shell_label_key(shell: TerminalShellType) -> &'static str {
    match shell {
        TerminalShellType::Auto => "connection.terminalShellOptions.auto",
        TerminalShellType::Cmd => "connection.terminalShellOptions.cmd",
        TerminalShellType::Powershell => "connection.terminalShellOptions.powershell",
        TerminalShellType::Pwsh => "connection.terminalShellOptions.pwsh",
        TerminalShellType::Wsl => "connection.terminalShellOptions.wsl",
        TerminalShellType::GitBash => "connection.terminalShellOptions.gitBash",
        TerminalShellType::Custom => "connection.terminalShellOptions.custom",
    }
}

/// The label a detected shell shows: the profile's own label for a shell found
/// on this machine or for a WSL distribution, and the translated option label
/// for the shells the app picks itself. An id the catalogue has no option for
/// is translated under the frontend's `undefined` key, which falls back to the
/// profile's label.
pub fn terminal_shell_label(
    profile: &TerminalShellProfile,
    translate: impl Fn(&str, &str) -> String,
) -> String {
    if is_detected_custom_shell(profile)
        || profile
            .wsl_distro
            .as_deref()
            .is_some_and(|distro| !distro.is_empty())
    {
        return profile.label.clone();
    }
    match shell_type_from_id(&profile.shell) {
        Some(shell) => translate(shell_label_key(shell), &profile.label),
        None => translate(UNKNOWN_SHELL_LABEL_KEY, &profile.label),
    }
}
