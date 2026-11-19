//! The shells a local tab can start, from `src/lib/localShells.ts`.
//!
//! The settings name a default shell and list the shells the user added; the
//! lists a new tab shows are built from the shells detected on this machine and
//! those the user added. A shell kind is the saved id string a connection and
//! the config keep, so an id no client writes travels with the profile, the
//! choice and the connection unchanged;
//! [`TerminalShellType`](crate::workspace::tab::TerminalShellType) names the
//! catalogue entry of each known id.

pub mod connections;
pub mod default_shell;
pub mod entries;
pub mod saved;
pub mod shell_label;
#[cfg(test)]
mod tests;
pub mod types;

use tterm_terminal::js_text::js_trim;

use crate::terminal::pty::TerminalShellProfile;
use crate::workspace::tab::TerminalShellType;

/// The key of the entry that starts the platform's own default shell.
pub const AUTO_SHELL_KEY: &str = "auto";

/// The shell kind a saved id names; `None` for an id no client writes, which
/// keeps its own label instead of a catalogue entry.
pub fn shell_type_from_id(id: &str) -> Option<TerminalShellType> {
    match id {
        "auto" => Some(TerminalShellType::Auto),
        "cmd" => Some(TerminalShellType::Cmd),
        "powershell" => Some(TerminalShellType::Powershell),
        "pwsh" => Some(TerminalShellType::Pwsh),
        "wsl" => Some(TerminalShellType::Wsl),
        "git-bash" => Some(TerminalShellType::GitBash),
        "custom" => Some(TerminalShellType::Custom),
        _ => None,
    }
}

/// The key a detected shell is hidden by: its kind on Windows, or a WSL
/// distribution's name, and its path elsewhere.
pub fn detected_shell_key(profile: &TerminalShellProfile) -> String {
    if let Some(distro) = profile
        .wsl_distro
        .as_deref()
        .filter(|distro| !distro.is_empty())
    {
        return wsl_key(distro);
    }
    if profile.shell == "custom" {
        profile.source.clone()
    } else {
        profile.shell.clone()
    }
}

/// The key of one of the user's own shells in the lists.
pub fn custom_shell_key(id: &str) -> String {
    format!("custom:{id}")
}

/// The program a path names, without directories or extension: `nu` for
/// `C:\nu\nu.exe`.
pub fn program_name(path: &str) -> String {
    let file = strip_one_quote(js_trim(path));
    let file = file.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = match file.rsplit_once('.') {
        Some((stem, extension)) if !extension.is_empty() => stem,
        _ => file,
    };
    if stem.is_empty() {
        file.to_string()
    } else {
        stem.to_string()
    }
}

/// The key of the WSL distribution `name` in the lists.
pub(crate) fn wsl_key(name: &str) -> String {
    format!("wsl:{name}")
}

/// The `wsl.exe` arguments that start a distribution.
pub(crate) fn wsl_distro_args(name: &str) -> String {
    format!("-d {name}")
}

/// A path without one leading and one trailing double quote, as
/// `path.replace(/^"|"$/g, "")` removes them.
pub(crate) fn strip_one_quote(path: &str) -> &str {
    let without_leading = path.strip_prefix('"').unwrap_or(path);
    without_leading.strip_suffix('"').unwrap_or(without_leading)
}

/// Whether two paths name the same file, ignoring surrounding whitespace.
pub(crate) fn same_path(left: &str, right: &str) -> bool {
    js_trim(left) == js_trim(right)
}
