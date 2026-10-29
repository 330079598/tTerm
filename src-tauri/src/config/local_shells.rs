use serde::{Deserialize, Serialize};

use super::AppConfig;

/// The id given to the custom shell an older version kept as the default,
/// the same on every load so tabs saved with it still find it.
const MIGRATED_SHELL_ID: &str = "default";

/// A shell the user added for local tabs. The frontend turns it into the
/// path and arguments a tab starts; this only keeps what it saved.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct LocalShell {
    pub id: String,
    pub name: String,
    pub path: String,
    pub args: String,
}

/// Turns the one custom shell older versions kept as the default into the
/// first entry of the list, chosen as the default, so it stays where the
/// user put it. The default's path and arguments stay as they were, which
/// older versions still read.
pub(super) fn migrate(config: &mut AppConfig) {
    let path = config.terminal_shell_custom_path.trim();
    if config.terminal_shell != "custom"
        || path.is_empty()
        || !config.terminal_shell_id.is_empty()
        || !config.local_shells.is_empty()
    {
        return;
    }
    config.local_shells.push(LocalShell {
        id: MIGRATED_SHELL_ID.to_string(),
        name: program_name(path),
        path: path.to_string(),
        args: config.terminal_shell_custom_args.trim().to_string(),
    });
    config.terminal_shell_id = MIGRATED_SHELL_ID.to_string();
}

/// The program's file name without its extension, `nu` for `C:\nu\nu.exe`.
fn program_name(path: &str) -> String {
    let path = path.trim_matches('"');
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem).trim();
    if stem.is_empty() { file } else { stem }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom_default(path: &str, args: &str) -> AppConfig {
        AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: path.to_string(),
            terminal_shell_custom_args: args.to_string(),
            ..AppConfig::default()
        }
    }

    #[test]
    fn keeps_an_older_custom_default_as_the_first_shell() {
        let mut config = custom_default(r#""C:\Tools\nu\nu.exe""#, " --login ");
        migrate(&mut config);
        assert_eq!(
            config.local_shells,
            vec![LocalShell {
                id: "default".to_string(),
                name: "nu".to_string(),
                path: r#""C:\Tools\nu\nu.exe""#.to_string(),
                args: "--login".to_string(),
            }]
        );
        assert_eq!(config.terminal_shell_id, "default");
        assert_eq!(config.terminal_shell_custom_args, " --login ");
    }

    #[test]
    fn leaves_other_configs_alone() {
        let mut auto = AppConfig::default();
        migrate(&mut auto);
        assert!(auto.local_shells.is_empty());

        let mut listed = custom_default("/bin/zsh", "");
        listed.terminal_shell_id = "a1".to_string();
        migrate(&mut listed);
        assert!(listed.local_shells.is_empty());

        let mut empty = custom_default("  ", "");
        migrate(&mut empty);
        assert!(empty.local_shells.is_empty());
    }

    #[test]
    fn names_shells_by_program() {
        assert_eq!(program_name("/opt/homebrew/bin/fish"), "fish");
        assert_eq!(program_name(r"C:\msys64\usr\bin\bash.exe"), "bash");
        assert_eq!(program_name("nu"), "nu");
        assert_eq!(program_name(".hidden"), ".hidden");
    }
}
