//! The settings, choices, list entries and updates of the local shell lists,
//! from `src/lib/localShells.ts`.

use crate::config::AppConfig;
use crate::config::local_shells::LocalShell;

use super::AUTO_SHELL_KEY;

/// The settings that say which shells local tabs can start and which is the
/// default, borrowed from an [`AppConfig`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellConfig<'a> {
    /// The kind of shell a local tab starts: `auto`, `cmd`, `powershell`,
    /// `pwsh`, `wsl`, `git-bash` or `custom`.
    pub terminal_shell: &'a str,
    /// The path of a `custom` shell; older versions read the default's path here.
    pub terminal_shell_custom_path: &'a str,
    /// The arguments of a `custom` shell.
    pub terminal_shell_custom_args: &'a str,
    /// The user's own shell the default is, when it is one of theirs.
    pub terminal_shell_id: &'a str,
    /// The WSL distribution the default starts when it is `wsl`; empty starts
    /// WSL's own default.
    pub terminal_shell_wsl_distro: &'a str,
    /// The shells the user added.
    pub local_shells: &'a [LocalShell],
    /// The keys of the detected shells the user hid.
    pub hidden_shells: &'a [String],
}

impl Default for ShellConfig<'_> {
    /// The settings a new config starts from: the platform's default shell and
    /// no shells of the user's own, as `AppConfig::default()` has them.
    fn default() -> Self {
        Self {
            terminal_shell: AUTO_SHELL_KEY,
            terminal_shell_custom_path: "",
            terminal_shell_custom_args: "",
            terminal_shell_id: "",
            terminal_shell_wsl_distro: "",
            local_shells: &[],
            hidden_shells: &[],
        }
    }
}

impl<'a> From<&'a AppConfig> for ShellConfig<'a> {
    fn from(config: &'a AppConfig) -> Self {
        Self {
            terminal_shell: &config.terminal_shell,
            terminal_shell_custom_path: &config.terminal_shell_custom_path,
            terminal_shell_custom_args: &config.terminal_shell_custom_args,
            terminal_shell_id: &config.terminal_shell_id,
            terminal_shell_wsl_distro: &config.terminal_shell_wsl_distro,
            local_shells: &config.local_shells,
            hidden_shells: &config.hidden_shells,
        }
    }
}

impl<'a> ShellConfig<'a> {
    /// These settings with the fields that name the default shell replaced, as
    /// the frontend spreads one over the other.
    pub fn with_default<'b>(&'b self, default: &'b DefaultShellSettings) -> ShellConfig<'b> {
        ShellConfig {
            terminal_shell: &default.terminal_shell,
            terminal_shell_custom_path: &default.terminal_shell_custom_path,
            terminal_shell_custom_args: &default.terminal_shell_custom_args,
            terminal_shell_id: &default.terminal_shell_id,
            terminal_shell_wsl_distro: &default.terminal_shell_wsl_distro,
            local_shells: self.local_shells,
            hidden_shells: self.hidden_shells,
        }
    }
}

/// The part of the settings that names the default shell, as the config keeps
/// it: every field is written, empty ones included.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultShellSettings {
    /// The kind of shell a local tab starts.
    pub terminal_shell: String,
    /// The path of a `custom` shell.
    pub terminal_shell_custom_path: String,
    /// The arguments of a `custom` shell.
    pub terminal_shell_custom_args: String,
    /// The user's own shell the default is, when it is one of theirs.
    pub terminal_shell_id: String,
    /// The WSL distribution a `wsl` default starts.
    pub terminal_shell_wsl_distro: String,
}

/// A shell a local tab can start, as a tab is opened with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalShellChoice {
    /// The saved id of the shell kind, as a tab's connection and the config
    /// keep it: `auto`, `cmd`, `powershell`, `pwsh`, `wsl`, `git-bash`,
    /// `custom`, or an id no client writes, which is carried through unchanged.
    pub shell: String,
    /// The executable of a `custom` shell.
    pub custom_path: Option<String>,
    /// The arguments of a `custom` shell.
    pub custom_args: Option<String>,
    /// The user's own shell this is, so a restored tab follows its later edits.
    pub shell_id: Option<String>,
    /// The WSL distribution a `wsl` shell starts; WSL's default when missing.
    pub wsl_distro: Option<String>,
}

impl Default for LocalShellChoice {
    /// The platform's own default shell.
    fn default() -> Self {
        Self {
            shell: AUTO_SHELL_KEY.to_string(),
            custom_path: None,
            custom_args: None,
            shell_id: None,
            wsl_distro: None,
        }
    }
}

/// Which list a shell entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEntryKind {
    /// The platform's own default shell.
    Auto,
    /// A shell found on this machine.
    Detected,
    /// A shell the user added.
    Custom,
}

/// One shell in the lists: Auto, one found on this machine, or one the user
/// added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellEntry {
    /// `auto`, a detected shell's key (`pwsh`, `wsl:Ubuntu`, or its path), or
    /// `custom:<id>`.
    pub key: String,
    /// Which list the entry belongs to.
    pub kind: ShellEntryKind,
    /// The translated label.
    pub label: String,
    /// The executable, with the arguments of a shell the user added.
    pub detail: String,
    /// The executable alone, or nothing for Auto.
    pub source: String,
    /// The shell the entry starts.
    pub choice: LocalShellChoice,
    /// Whether the user hid this detected shell.
    pub hidden: bool,
}

/// The program and arguments a shell of the user's own copied from a detected
/// entry starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedShellCommand {
    /// The executable to run.
    pub path: String,
    /// The arguments to run it with.
    pub args: String,
}

/// What a local tab whose shell failed to start can open instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedShellFallback {
    /// The shell to open the tab with.
    pub choice: LocalShellChoice,
    /// Whether this is Auto standing in for a default shell that failed
    /// itself, so a broken default still leaves a way out.
    pub replaces_default: bool,
}

/// How a change to the user's shells moves the default shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultShellChange {
    /// The default shell's own path and arguments follow its edits; its kind,
    /// id and WSL distribution stay as they are.
    Edited {
        /// The edited shell's path, trimmed.
        path: String,
        /// The edited shell's arguments, trimmed.
        args: String,
    },
    /// Another shell becomes the default.
    Chosen(DefaultShellSettings),
}

/// The shells the user has after a change, and the default shell it also
/// moves, which the caller writes back into the config.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellListUpdate {
    /// The shells to save, in the order the lists show them.
    pub local_shells: Vec<LocalShell>,
    /// The default shell the change moves; `None` leaves it as it is.
    pub default_shell: Option<DefaultShellChange>,
}
