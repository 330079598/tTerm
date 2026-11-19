//! Which shell is the default, and how a choice names it, from
//! `src/lib/localShells.ts`.

use tterm_terminal::js_text::js_trim;

use super::saved::own_shell_choice;
use super::types::{DefaultShellSettings, LocalShellChoice, ShellConfig};
use super::{custom_shell_key, same_path, wsl_key};

/// The key of the default shell among the entries a new tab lists.
pub fn default_shell_key(config: &ShellConfig<'_>) -> String {
    let id = config.terminal_shell_id;
    if !id.is_empty() && config.local_shells.iter().any(|shell| shell.id == id) {
        return custom_shell_key(id);
    }
    let distro = js_trim(config.terminal_shell_wsl_distro);
    if config.terminal_shell == "wsl" && !distro.is_empty() {
        return wsl_key(distro);
    }
    if config.terminal_shell == "custom" {
        let path = js_trim(config.terminal_shell_custom_path);
        let added = config
            .local_shells
            .iter()
            .find(|shell| js_trim(&shell.args).is_empty() && same_path(&shell.path, path));
        return added.map_or_else(|| path.to_string(), |shell| custom_shell_key(&shell.id));
    }
    config.terminal_shell.to_string()
}

/// The key of `choice` among the entries a new tab lists, as if it were the
/// default.
pub fn shell_choice_key(choice: &LocalShellChoice, config: &ShellConfig<'_>) -> String {
    let default = default_shell_updates(choice);
    default_shell_key(&config.with_default(&default))
}

/// The settings that make `choice` the default shell.
pub fn default_shell_updates(choice: &LocalShellChoice) -> DefaultShellSettings {
    let custom = choice.shell == "custom";
    let wsl = choice.shell == "wsl";
    DefaultShellSettings {
        terminal_shell: choice.shell.clone(),
        terminal_shell_custom_path: if custom {
            js_trim(choice.custom_path.as_deref().unwrap_or_default()).to_string()
        } else {
            String::new()
        },
        terminal_shell_custom_args: if custom {
            js_trim(choice.custom_args.as_deref().unwrap_or_default()).to_string()
        } else {
            String::new()
        },
        terminal_shell_id: if custom {
            choice.shell_id.clone().unwrap_or_default()
        } else {
            String::new()
        },
        terminal_shell_wsl_distro: if wsl {
            js_trim(choice.wsl_distro.as_deref().unwrap_or_default()).to_string()
        } else {
            String::new()
        },
    }
}

/// The default shell, with the current path of a shell the user added.
pub fn default_shell_choice(config: &ShellConfig<'_>) -> LocalShellChoice {
    if config.terminal_shell == "custom"
        && let Some(added) = config
            .local_shells
            .iter()
            .find(|shell| shell.id == config.terminal_shell_id)
    {
        return own_shell_choice(added);
    }
    let custom = config.terminal_shell == "custom";
    let wsl = config.terminal_shell == "wsl";
    LocalShellChoice {
        shell: config.terminal_shell.to_string(),
        custom_path: custom.then(|| js_trim(config.terminal_shell_custom_path).to_string()),
        custom_args: custom
            .then(|| js_trim(config.terminal_shell_custom_args).to_string())
            .filter(|args| !args.is_empty()),
        wsl_distro: wsl
            .then(|| js_trim(config.terminal_shell_wsl_distro).to_string())
            .filter(|distro| !distro.is_empty()),
        shell_id: None,
    }
}
