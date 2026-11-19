//! The entries a new local tab lists, from `src/lib/localShells.ts`.

use tterm_terminal::js_text::js_trim;

use crate::config::local_shells::LocalShell;
use crate::terminal::pty::TerminalShellProfile;
use crate::workspace::tab::TerminalShellType;

use super::saved::own_shell_choice;
use super::shell_label;
use super::types::{CopiedShellCommand, LocalShellChoice, ShellConfig, ShellEntry, ShellEntryKind};
use super::{AUTO_SHELL_KEY, custom_shell_key, detected_shell_key, program_name, wsl_distro_args};

/// The label Auto shows until the catalogue is translated.
const AUTO_LABEL: &str = "Auto (recommended)";

/// The label a default of WSL's own distribution shows until translated.
const WSL_LABEL: &str = "Windows Subsystem for Linux (WSL)";

/// Every shell a local tab can start, in the order the lists show them: Auto,
/// the shells found on this machine, then the ones the user added. A detected
/// shell the user also added as it is shows once, as theirs.
///
/// A default of WSL's own default distribution, from older versions, is listed
/// as an entry of its own ahead of the distributions: it follows whichever one
/// WSL makes its default, and keeps their keys the same either way. It cannot
/// be hidden while it is the default, and it is listed only then.
pub fn build_shell_entries(
    detected: &[TerminalShellProfile],
    config: &ShellConfig<'_>,
    translate: impl Fn(&str, &str) -> String,
) -> Vec<ShellEntry> {
    let mut entries = vec![ShellEntry {
        key: AUTO_SHELL_KEY.to_string(),
        kind: ShellEntryKind::Auto,
        label: translate(
            shell_label::shell_label_key(TerminalShellType::Auto),
            AUTO_LABEL,
        ),
        detail: String::new(),
        source: String::new(),
        choice: LocalShellChoice::default(),
        hidden: false,
    }];
    let mut wsl_default =
        config.terminal_shell == "wsl" && js_trim(config.terminal_shell_wsl_distro).is_empty();
    let added_as_is = |path: &str| {
        config
            .local_shells
            .iter()
            .any(|shell| js_trim(&shell.args).is_empty() && super::same_path(&shell.path, path))
    };
    for profile in detected {
        if profile.shell == AUTO_SHELL_KEY {
            continue;
        }
        let custom = profile.shell == "custom";
        if custom
            && (!shell_label::is_detected_custom_shell(profile) || added_as_is(&profile.source))
        {
            continue;
        }
        let distro = profile
            .wsl_distro
            .as_deref()
            .filter(|distro| !distro.is_empty());
        if wsl_default && distro.is_some() {
            wsl_default = false;
            entries.push(wsl_default_entry(profile, config, &translate));
        }
        let key = detected_shell_key(profile);
        entries.push(ShellEntry {
            hidden: config.hidden_shells.contains(&key),
            key,
            kind: ShellEntryKind::Detected,
            label: shell_label::terminal_shell_label(profile, &translate),
            detail: match distro {
                Some(distro) => format!("{} {}", profile.source, wsl_distro_args(distro)),
                None => profile.source.clone(),
            },
            source: profile.source.clone(),
            choice: LocalShellChoice {
                shell: profile.shell.clone(),
                custom_path: custom.then(|| profile.source.clone()),
                wsl_distro: profile.wsl_distro.clone(),
                ..LocalShellChoice::default()
            },
        });
    }
    for shell in config.local_shells {
        entries.push(own_shell_entry(shell));
    }
    entries
}

/// WSL read as a whole, for a default of WSL's own default distribution.
fn wsl_default_entry(
    profile: &TerminalShellProfile,
    config: &ShellConfig<'_>,
    translate: &impl Fn(&str, &str) -> String,
) -> ShellEntry {
    ShellEntry {
        key: "wsl".to_string(),
        kind: ShellEntryKind::Detected,
        label: translate(
            shell_label::shell_label_key(TerminalShellType::Wsl),
            WSL_LABEL,
        ),
        detail: profile.source.clone(),
        source: profile.source.clone(),
        choice: LocalShellChoice {
            shell: "wsl".to_string(),
            ..LocalShellChoice::default()
        },
        hidden: config.hidden_shells.iter().any(|key| key == "wsl"),
    }
}

/// One of the user's own shells, as the lists show it.
fn own_shell_entry(shell: &LocalShell) -> ShellEntry {
    let name = js_trim(&shell.name);
    ShellEntry {
        key: custom_shell_key(&shell.id),
        kind: ShellEntryKind::Custom,
        label: if name.is_empty() {
            program_name(&shell.path)
        } else {
            name.to_string()
        },
        detail: describe_shell(shell),
        source: js_trim(&shell.path).to_string(),
        choice: own_shell_choice(shell),
        hidden: false,
    }
}

/// The executable of a shell the user added, with its arguments.
fn describe_shell(shell: &LocalShell) -> String {
    let args = js_trim(&shell.args);
    if args.is_empty() {
        js_trim(&shell.path).to_string()
    } else {
        format!("{} {args}", js_trim(&shell.path))
    }
}

/// The program and arguments a shell of the user's own copied from a detected
/// `entry` starts.
pub fn copied_shell_command(entry: &ShellEntry) -> CopiedShellCommand {
    let args = entry
        .choice
        .wsl_distro
        .as_deref()
        .filter(|distro| !distro.is_empty())
        .map(wsl_distro_args)
        .unwrap_or_default();
    CopiedShellCommand {
        path: entry.source.clone(),
        args,
    }
}
