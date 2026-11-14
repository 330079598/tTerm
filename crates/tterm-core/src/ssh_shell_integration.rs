//! A connection's shell integration choice and when a fallback is announced,
//! from `src/lib/sshShellIntegration.ts`.

use crate::events::ssh::UnavailableReason;

/// A connection's shell integration as its form shows it: `default` follows
/// the setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIntegrationChoice {
    /// Follow the app setting.
    Default,
    /// Turn shell integration on for this connection.
    On,
    /// Turn shell integration off for this connection.
    Off,
}

impl ShellIntegrationChoice {
    /// The stored or sent form of the choice.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

/// The form's choice for a stored or sent value, where missing follows the
/// setting.
pub fn shell_integration_choice(value: Option<bool>) -> ShellIntegrationChoice {
    match value {
        None => ShellIntegrationChoice::Default,
        Some(true) => ShellIntegrationChoice::On,
        Some(false) => ShellIntegrationChoice::Off,
    }
}

/// The form's choice as stored or sent: unset follows the setting.
pub fn shell_integration_payload(choice: ShellIntegrationChoice) -> Option<bool> {
    match choice {
        ShellIntegrationChoice::Default => None,
        ShellIntegrationChoice::On => Some(true),
        ShellIntegrationChoice::Off => Some(false),
    }
}

/// Whether an SSH connection's sessions start with tTerm's shell integration:
/// its own choice, or the setting when it has none. The backend resolves it
/// the same way (`normalize_ssh` in `src-tauri/src/core/session/mod.rs`).
pub fn ssh_shell_integration_enabled(choice: Option<bool>, setting: bool) -> bool {
    choice.unwrap_or(setting)
}

/// Whether a session that fell back to a plain shell should say so in a toast.
///
/// A host that answered for itself (another shell, no `sh`, a cache it cannot
/// write) answers the same every time, which is news only to a connection that
/// turned shell integration on itself; with the setting on by default, a
/// router or a Windows server would otherwise raise a toast on every connect.
/// The connection header still says why.
pub fn announces_shell_integration_fallback(
    choice: Option<bool>,
    reason: &UnavailableReason,
) -> bool {
    if choice == Some(true) {
        return true;
    }
    match reason {
        UnavailableReason::Shell { .. }
        | UnavailableReason::NoReply
        | UnavailableReason::WriteFailed
        | UnavailableReason::Path => false,
        UnavailableReason::Rejected | UnavailableReason::TimedOut | UnavailableReason::Failed => {
            true
        }
    }
}

#[cfg(test)]
mod tests;
