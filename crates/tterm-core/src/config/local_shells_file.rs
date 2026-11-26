//! Persists one local-shell edit without replacing other settings with a startup snapshot.
//!
//! The edit starts from the latest `config.json` rather than the in-memory
//! snapshot, applies the shared local-shell rules, and writes the result back
//! through the compatible-file writer. A setting another client changed in the
//! meantime, or a key this version does not know, therefore survives the save.

use std::path::Path;

use super::AppConfig;
use super::local_shells::LocalShell;
use super::paths::ensure_config_dir;
use crate::workspace::local_shells::AUTO_SHELL_KEY;
use crate::workspace::local_shells::default_shell::default_shell_key;
use crate::workspace::local_shells::default_shell::default_shell_updates;
use crate::workspace::local_shells::saved::moved_local_shell;
use crate::workspace::local_shells::saved::own_shell_choice;
use crate::workspace::local_shells::saved::removed_local_shell;
use crate::workspace::local_shells::saved::saved_local_shell;
use crate::workspace::local_shells::types::{
    DefaultShellChange, DefaultShellSettings, LocalShellChoice, ShellConfig,
};

/// The shell-related settings after a save, already read back from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedLocalShells {
    /// The complete saved list, in display order.
    pub local_shells: Vec<LocalShell>,
    /// The default shell settings the save also moved, when it did.
    pub default_shell: DefaultShellSettings,
}

/// The shell settings after a list edit, already read back from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedShellLists {
    /// The complete saved list, in display order.
    pub local_shells: Vec<LocalShell>,
    /// The keys of the detected shells the user hid.
    pub hidden_shells: Vec<String>,
    /// The default shell settings the edit also moved, when it did.
    pub default_shell: DefaultShellSettings,
}

impl From<&AppConfig> for SavedShellLists {
    fn from(config: &AppConfig) -> Self {
        Self {
            local_shells: config.local_shells.clone(),
            hidden_shells: config.hidden_shells.clone(),
            default_shell: default_shell_settings(config),
        }
    }
}

/// One change to the local-shell lists, applied to the latest config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellListEdit {
    /// Show or hide one detected shell by its key.
    Visibility {
        /// The key the lists give the detected shell.
        key: String,
        /// The desired visibility; showing a shell that is not hidden is a no-op.
        hidden: bool,
    },
    /// Move one of the user's shells within the list.
    Move {
        /// The saved id of the shell to move.
        id: String,
        /// `-1` moves it one place up and `1` one place down.
        offset: i32,
    },
    /// Remove one of the user's shells.
    Remove {
        /// The saved id of the shell to remove.
        id: String,
    },
}

/// Adds or replaces one local shell in the shared config file.
pub fn save_local_shell(shell: LocalShell) -> Result<SavedLocalShells, String> {
    save_at(&ensure_config_dir()?.join("config.json"), shell)
}

/// Applies one list edit to the shared config file.
pub fn edit_shell_list(edit: ShellListEdit) -> Result<SavedShellLists, String> {
    edit_shell_list_at(&ensure_config_dir()?.join("config.json"), edit)
}

/// Saves one default-shell choice in the shared config file.
pub fn save_default_shell(choice: LocalShellChoice) -> Result<SavedLocalShells, String> {
    save_default_at(&ensure_config_dir()?.join("config.json"), choice)
}

pub(super) fn save_at(path: &Path, shell: LocalShell) -> Result<SavedLocalShells, String> {
    let _lock = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    let update = saved_local_shell(&ShellConfig::from(&config), &shell);
    config.local_shells = update.local_shells;
    if let Some(change) = update.default_shell {
        match change {
            DefaultShellChange::Edited { path, args } => {
                config.terminal_shell_custom_path = path;
                config.terminal_shell_custom_args = args;
            }
            DefaultShellChange::Chosen(default) => apply_default_shell(&mut config, default),
        }
    }
    super::compatible_file::save_at(path, &config)?;
    let default_shell = default_shell_settings(&config);
    Ok(SavedLocalShells {
        local_shells: config.local_shells,
        default_shell,
    })
}

pub(super) fn edit_shell_list_at(
    path: &Path,
    edit: ShellListEdit,
) -> Result<SavedShellLists, String> {
    let _lock = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    apply_shell_list_edit(&mut config, &edit)?;
    super::compatible_file::save_at(path, &config)?;
    Ok(SavedShellLists::from(&config))
}

pub(super) fn save_default_at(
    path: &Path,
    choice: LocalShellChoice,
) -> Result<SavedLocalShells, String> {
    let _lock = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    let default = default_shell_from_latest(&config, choice)?;
    apply_default_shell(&mut config, default);
    super::compatible_file::save_at(path, &config)?;
    let default_shell = default_shell_settings(&config);
    Ok(SavedLocalShells {
        local_shells: config.local_shells,
        default_shell,
    })
}

/// Applies `edit` to `config`, rejecting an edit the lists cannot take.
///
/// The file writer and the isolated writer the tests use both call this, so
/// an edit is decided the same way whether it reaches a config file or not.
pub fn apply_shell_list_edit(config: &mut AppConfig, edit: &ShellListEdit) -> Result<(), String> {
    match edit {
        ShellListEdit::Visibility { key, hidden } => apply_visibility(config, key, *hidden),
        ShellListEdit::Move { id, offset } => {
            if !matches!(*offset, -1 | 1) {
                return Err("A local shell moves one place up or down.".to_string());
            }
            config.local_shells = moved_local_shell(&config.local_shells, id, *offset);
            Ok(())
        }
        ShellListEdit::Remove { id } => {
            let update = removed_local_shell(&ShellConfig::from(&*config), id);
            config.local_shells = update.local_shells;
            if let Some(DefaultShellChange::Chosen(default)) = update.default_shell {
                apply_default_shell(config, default);
            }
            Ok(())
        }
    }
}

/// Sets one detected shell's visibility, refusing a shell the lists must keep.
///
/// A shell that is already hidden keeps its place and is not checked again, so
/// asking twice for the same visibility always succeeds.
fn apply_visibility(config: &mut AppConfig, key: &str, hidden: bool) -> Result<(), String> {
    if key.is_empty() {
        return Err("A local shell key is required to change its visibility.".to_string());
    }
    let already_hidden = config.hidden_shells.iter().any(|other| other == key);
    if hidden && !already_hidden {
        if key == AUTO_SHELL_KEY {
            return Err("Auto cannot be hidden.".to_string());
        }
        if key.starts_with("custom:") {
            return Err("A shell you added cannot be hidden. Remove it instead.".to_string());
        }
        if default_shell_key(&ShellConfig::from(&*config)) == key {
            return Err(
                "The default shell cannot be hidden. Choose another default shell first."
                    .to_string(),
            );
        }
    }
    config.hidden_shells = desired_hidden(&config.hidden_shells, key, hidden);
    Ok(())
}

/// The hidden keys with `key` present or absent as asked, so asking twice for
/// the same visibility leaves the list as it is.
fn desired_hidden(current: &[String], key: &str, hidden: bool) -> Vec<String> {
    if hidden {
        if current.iter().any(|other| other == key) {
            return current.to_vec();
        }
        let mut next = current.to_vec();
        next.push(key.to_string());
        return next;
    }
    current
        .iter()
        .filter(|other| other.as_str() != key)
        .cloned()
        .collect()
}

/// The default settings a choice names, using the latest path and arguments of
/// one of the user's own shells. A shell that another edit removed cannot
/// become the default.
fn default_shell_from_latest(
    config: &AppConfig,
    choice: LocalShellChoice,
) -> Result<DefaultShellSettings, String> {
    if choice.shell == "custom"
        && let Some(id) = choice.shell_id.as_deref().filter(|id| !id.is_empty())
    {
        let shell = config
            .local_shells
            .iter()
            .find(|shell| shell.id == id)
            .ok_or_else(|| {
                "The selected local shell was removed. Choose another shell and try again."
                    .to_string()
            })?;
        return Ok(default_shell_updates(&own_shell_choice(shell)));
    }
    Ok(default_shell_updates(&choice))
}

fn apply_default_shell(config: &mut AppConfig, default: DefaultShellSettings) {
    config.terminal_shell = default.terminal_shell;
    config.terminal_shell_custom_path = default.terminal_shell_custom_path;
    config.terminal_shell_custom_args = default.terminal_shell_custom_args;
    config.terminal_shell_id = default.terminal_shell_id;
    config.terminal_shell_wsl_distro = default.terminal_shell_wsl_distro;
}

fn default_shell_settings(config: &AppConfig) -> DefaultShellSettings {
    DefaultShellSettings {
        terminal_shell: config.terminal_shell.clone(),
        terminal_shell_custom_path: config.terminal_shell_custom_path.clone(),
        terminal_shell_custom_args: config.terminal_shell_custom_args.clone(),
        terminal_shell_id: config.terminal_shell_id.clone(),
        terminal_shell_wsl_distro: config.terminal_shell_wsl_distro.clone(),
    }
}

#[cfg(test)]
mod tests;
