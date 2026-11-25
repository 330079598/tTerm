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
use crate::workspace::local_shells::saved::saved_local_shell;
use crate::workspace::local_shells::types::{
    DefaultShellChange, DefaultShellSettings, ShellConfig,
};

/// The shell-related settings after a save, already read back from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedLocalShells {
    /// The complete saved list, in display order.
    pub local_shells: Vec<LocalShell>,
    /// The default shell settings the save also moved, when it did.
    pub default_shell: DefaultShellSettings,
}

/// Adds or replaces one local shell in the shared config file.
pub fn save_local_shell(shell: LocalShell) -> Result<SavedLocalShells, String> {
    save_at(&ensure_config_dir()?.join("config.json"), shell)
}

pub(super) fn save_at(path: &Path, shell: LocalShell) -> Result<SavedLocalShells, String> {
    let _edit = super::edit::lock()?;
    let mut config = super::load_config_at(path)?;
    let update = saved_local_shell(&ShellConfig::from(&config), &shell);
    config.local_shells = update.local_shells;
    if let Some(change) = update.default_shell {
        match change {
            DefaultShellChange::Edited { path, args } => {
                config.terminal_shell_custom_path = path;
                config.terminal_shell_custom_args = args;
            }
            DefaultShellChange::Chosen(default) => {
                config.terminal_shell = default.terminal_shell;
                config.terminal_shell_custom_path = default.terminal_shell_custom_path;
                config.terminal_shell_custom_args = default.terminal_shell_custom_args;
                config.terminal_shell_id = default.terminal_shell_id;
                config.terminal_shell_wsl_distro = default.terminal_shell_wsl_distro;
            }
        }
    }
    super::compatible_file::save_at(path, &config)?;
    let default_shell = default_shell_settings(&config);
    Ok(SavedLocalShells {
        local_shells: config.local_shells,
        default_shell,
    })
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
mod tests {
    use std::fs;

    use super::*;

    const SHELL_ID: &str = "shell-nu";

    fn temp_config() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("config-{}.json", uuid::Uuid::new_v4()))
    }

    fn shell(path: &str, args: &str) -> LocalShell {
        LocalShell {
            id: SHELL_ID.to_string(),
            name: "nu".to_string(),
            path: path.to_string(),
            args: args.to_string(),
        }
    }

    #[test]
    fn writes_into_the_latest_config_without_resetting_unrelated_settings() {
        let path = temp_config();
        let seeded = AppConfig {
            font_size: 24,
            scrollback_lines: 372,
            ..AppConfig::default()
        };
        super::super::compatible_file::save_at(&path, &seeded).unwrap();
        let extended =
            fs::read_to_string(&path)
                .unwrap()
                .replacen('{', r#"{"future":1.2300e+45,"#, 1);
        fs::write(&path, extended).unwrap();

        let saved = save_at(&path, shell("/opt/homebrew/bin/nu", "--login")).unwrap();

        assert_eq!(
            saved.local_shells,
            vec![shell("/opt/homebrew/bin/nu", "--login")]
        );
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!((reloaded.font_size, reloaded.scrollback_lines), (24, 372));
        assert_eq!(reloaded.local_shells, saved.local_shells);
        assert!(fs::read_to_string(&path).unwrap().contains("1.2300e+45"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn edits_a_listed_shell_in_place() {
        let path = temp_config();
        let first = shell("/bin/fish", "");
        let second = LocalShell {
            id: "shell-zsh".to_string(),
            name: "zsh".to_string(),
            path: "/bin/zsh".to_string(),
            args: "-l".to_string(),
        };
        super::super::compatible_file::save_at(
            &path,
            &AppConfig {
                local_shells: vec![first, second.clone()],
                ..AppConfig::default()
            },
        )
        .unwrap();

        let saved = save_at(&path, shell("/opt/homebrew/bin/nu", "-l")).unwrap();

        assert_eq!(
            saved.local_shells,
            vec![shell("/opt/homebrew/bin/nu", "-l"), second]
        );
        assert_eq!(
            super::super::load_config_at(&path).unwrap().local_shells,
            saved.local_shells
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn keeps_the_default_path_in_step_with_an_edited_default_shell() {
        let path = temp_config();
        super::super::compatible_file::save_at(
            &path,
            &AppConfig {
                terminal_shell: "custom".to_string(),
                terminal_shell_id: SHELL_ID.to_string(),
                terminal_shell_custom_path: "/bin/nu".to_string(),
                local_shells: vec![shell("/bin/nu", "")],
                ..AppConfig::default()
            },
        )
        .unwrap();

        let saved = save_at(&path, shell("/opt/homebrew/bin/nu", "-l")).unwrap();

        assert_eq!(saved.default_shell.terminal_shell, "custom");
        assert_eq!(saved.default_shell.terminal_shell_id, SHELL_ID);
        assert_eq!(
            saved.default_shell.terminal_shell_custom_path,
            "/opt/homebrew/bin/nu"
        );
        assert_eq!(saved.default_shell.terminal_shell_custom_args, "-l");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn creates_a_complete_config_when_none_exists() {
        let path = temp_config();
        let saved = save_at(&path, shell("/bin/nu", "")).unwrap();
        assert_eq!(saved.local_shells, vec![shell("/bin/nu", "")]);
        let reloaded = super::super::load_config_at(&path).unwrap();
        assert_eq!(reloaded.local_shells, saved.local_shells);
        assert_eq!(
            reloaded.scrollback_lines,
            AppConfig::default().scrollback_lines
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_an_unreadable_config() {
        let path = temp_config();
        for damaged in ["{", r#"{"theme":false}"#] {
            fs::write(&path, damaged).unwrap();
            assert!(save_at(&path, shell("/bin/nu", "")).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
        }
        fs::remove_file(path).unwrap();
    }
}
