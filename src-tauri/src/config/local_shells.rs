use serde::{Deserialize, Serialize};

use super::AppConfig;

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
    let args = config.terminal_shell_custom_args.trim();
    let id = migrated_shell_id(path, args);
    config.local_shells.push(LocalShell {
        id: id.clone(),
        name: program_name(path),
        path: path.to_string(),
        args: args.to_string(),
    });
    config.terminal_shell_id = id;
}

/// The id given to the custom shell an older version kept as the default.
/// It comes from the program and its arguments, so it is the same on every
/// load and tabs saved with it still find it, while a shell migrated on
/// another device gets an id of its own unless it is the same shell, and a
/// backup from there doesn't pass it off as this device's.
fn migrated_shell_id(path: &str, args: &str) -> String {
    // FNV-1a stays the same across Rust versions, which std's hasher doesn't promise.
    let bytes = [path.as_bytes(), &[0], args.as_bytes()];
    let hash = bytes
        .into_iter()
        .flatten()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("migrated-{hash:016x}")
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
        let id = migrated_shell_id(r#""C:\Tools\nu\nu.exe""#, "--login");
        assert_eq!(
            config.local_shells,
            vec![LocalShell {
                id: id.clone(),
                name: "nu".to_string(),
                path: r#""C:\Tools\nu\nu.exe""#.to_string(),
                args: "--login".to_string(),
            }]
        );
        assert_eq!(config.terminal_shell_id, id);
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
    fn gives_migrated_shells_the_same_id_only_for_the_same_shell() {
        let id = |path: &str, args: &str| {
            let mut config = custom_default(path, args);
            migrate(&mut config);
            config.terminal_shell_id
        };
        assert_eq!(id("/bin/zsh", "-l"), id("/bin/zsh", " -l "));
        assert_ne!(id("/bin/zsh", "-l"), id("/opt/homebrew/bin/fish", "-l"));
        assert_ne!(id("/bin/zsh", "-l"), id("/bin/zsh", ""));
        assert_ne!(id("/bin/zs", "h"), id("/bin/zsh", ""));
    }

    #[test]
    fn names_shells_by_program() {
        assert_eq!(program_name("/opt/homebrew/bin/fish"), "fish");
        assert_eq!(program_name(r"C:\msys64\usr\bin\bash.exe"), "bash");
        assert_eq!(program_name("nu"), "nu");
        assert_eq!(program_name(".hidden"), ".hidden");
    }
}
