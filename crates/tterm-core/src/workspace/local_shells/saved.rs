//! Saving, removing and reordering the shells the user added, from
//! `src/lib/localShells.ts`.

use tterm_terminal::js_text::js_trim;

use crate::config::local_shells::LocalShell;
use crate::workspace::tab::TabConnection;

use super::AUTO_SHELL_KEY;
use super::connections::starts_shell;
use super::default_shell::{default_shell_choice, default_shell_updates};
use super::types::{
    DefaultShellChange, FailedShellFallback, LocalShellChoice, ShellConfig, ShellListUpdate,
};

/// Starting one of the user's own shells.
pub fn own_shell_choice(shell: &LocalShell) -> LocalShellChoice {
    let args = js_trim(&shell.args);
    LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some(js_trim(&shell.path).to_string()),
        custom_args: (!args.is_empty()).then(|| args.to_string()),
        shell_id: Some(shell.id.clone()),
        wsl_distro: None,
    }
}

/// The settings with a shell of the user's added or, when its id is already
/// listed, replaced. The default's own path and arguments follow its edits,
/// for older versions.
pub fn saved_local_shell(config: &ShellConfig<'_>, shell: &LocalShell) -> ShellListUpdate {
    let listed = config
        .local_shells
        .iter()
        .any(|candidate| candidate.id == shell.id);
    let local_shells = if listed {
        config
            .local_shells
            .iter()
            .map(|candidate| {
                if candidate.id == shell.id {
                    shell.clone()
                } else {
                    candidate.clone()
                }
            })
            .collect()
    } else {
        let mut added = config.local_shells.to_vec();
        added.push(shell.clone());
        added
    };
    let default_shell = (config.terminal_shell == "custom" && config.terminal_shell_id == shell.id)
        .then(|| DefaultShellChange::Edited {
            path: js_trim(&shell.path).to_string(),
            args: js_trim(&shell.args).to_string(),
        });
    ShellListUpdate {
        local_shells,
        default_shell,
    }
}

/// The settings with `shell` saved, and made the default when asked.
pub fn saved_local_shell_updates(
    config: &ShellConfig<'_>,
    shell: &LocalShell,
    make_default: bool,
) -> ShellListUpdate {
    let mut update = saved_local_shell(config, shell);
    if make_default {
        update.default_shell = Some(DefaultShellChange::Chosen(default_shell_updates(
            &own_shell_choice(shell),
        )));
    }
    update
}

/// The settings without the user's shell `id`; Auto becomes the default if it
/// was.
pub fn removed_local_shell(config: &ShellConfig<'_>, id: &str) -> ShellListUpdate {
    let local_shells = config
        .local_shells
        .iter()
        .filter(|shell| shell.id != id)
        .cloned()
        .collect();
    let default_shell = (config.terminal_shell == "custom" && config.terminal_shell_id == id)
        .then(|| DefaultShellChange::Chosen(default_shell_updates(&LocalShellChoice::default())));
    ShellListUpdate {
        local_shells,
        default_shell,
    }
}

/// The user's shells with `id` moved one place up (`-1`) or down (`1`), and
/// unchanged at either end of the list.
pub fn moved_local_shell(shells: &[LocalShell], id: &str, offset: i32) -> Vec<LocalShell> {
    let Some(index) = shells.iter().position(|shell| shell.id == id) else {
        return shells.to_vec();
    };
    let Ok(target) = usize::try_from(index as i64 + i64::from(offset)) else {
        return shells.to_vec();
    };
    if target >= shells.len() {
        return shells.to_vec();
    }
    let mut moved = shells.to_vec();
    moved.swap(index, target);
    moved
}

/// The hidden shells with `key` shown again, or hidden.
pub fn toggled_hidden_shell(hidden: &[String], key: &str) -> Vec<String> {
    if hidden.iter().any(|other| other == key) {
        hidden
            .iter()
            .filter(|other| other.as_str() != key)
            .cloned()
            .collect()
    } else {
        let mut toggled = hidden.to_vec();
        toggled.push(key.to_string());
        toggled
    }
}

/// A new id for a shell the user adds.
pub fn new_local_shell_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The shell a tab whose shell failed to start can open instead: the default,
/// or Auto when the default is the shell that failed, so a broken default
/// still leaves a way out. Nothing when Auto itself failed.
pub fn failed_shell_fallback(
    connection: Option<&TabConnection>,
    config: &ShellConfig<'_>,
) -> Option<FailedShellFallback> {
    let fallback = default_shell_choice(config);
    if !starts_shell(connection, &fallback) {
        return Some(FailedShellFallback {
            choice: fallback,
            replaces_default: false,
        });
    }
    if fallback.shell == AUTO_SHELL_KEY {
        return None;
    }
    Some(FailedShellFallback {
        choice: LocalShellChoice::default(),
        replaces_default: true,
    })
}
