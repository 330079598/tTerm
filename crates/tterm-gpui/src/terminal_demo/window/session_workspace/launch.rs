//! Turns a saved tab into a launch the native window can perform safely.

use tterm_core::config::AppConfig;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::workspace::local_shells::connections::with_current_local_shell;
use tterm_core::workspace::local_shells::types::LocalShellChoice;
use tterm_core::workspace::tab::{ConnectionType, Tab, TabConnection, TabType};

use crate::terminal_demo::command::{LaunchSource, SshProfileSelector, TabLaunch};

const SHELL_FIELD: &str = "terminalShell";
const SHELL_PATH_FIELD: &str = "terminalShellCustomPath";
const SHELL_ARGS_FIELD: &str = "terminalShellCustomArgs";
const SHELL_ID_FIELD: &str = "terminalShellId";
#[cfg(windows)]
const SHELL_DISTRO_FIELD: &str = "terminalShellWslDistro";
const CWD_FIELD: &str = "cwd";

/// A saved tab as a safe launch, or as an inert placeholder that keeps its
/// saved fields for the next save.
pub(crate) fn launch_for_tab(tab: &Tab, config: &AppConfig) -> TabLaunch {
    match tab.tab_type {
        TabType::Terminal => match local_options(tab, config) {
            Ok(options) => {
                TabLaunch::restored_tab(tab.clone(), LaunchSource::Local(Box::new(options)))
            }
            Err(reason) => TabLaunch::unsupported(tab.clone(), reason),
        },
        TabType::Ssh => match ssh_selector(tab) {
            Some(selector) => {
                TabLaunch::restored_tab(tab.clone(), LaunchSource::SshProfile { selector })
            }
            None => TabLaunch::unsupported(
                tab.clone(),
                "The saved SSH tab has no saved connection to reopen safely.".to_string(),
            ),
        },
        _ => TabLaunch::unsupported(
            tab.clone(),
            format!(
                "The saved '{}' tab is not supported by the GPUI client yet.",
                String::from(tab.tab_type.clone())
            ),
        ),
    }
}

pub(in crate::terminal_demo::window) fn local_options(
    tab: &Tab,
    config: &AppConfig,
) -> Result<PtyConnectionOptions, String> {
    let connection = tab
        .connection
        .as_ref()
        .ok_or_else(|| "The saved local tab has no connection fields.".to_string())?;
    if connection.connection_type != Some(ConnectionType::Terminal) {
        return Err("The saved local tab is not a local shell.".to_string());
    }

    // The current settings are trusted; a shell id that no longer exists
    // leaves the saved path untouched and is handled below.
    let current = with_current_local_shell(Some(connection), &config.local_shells)
        .ok_or_else(|| "The saved local shell is unavailable.".to_string())?;
    let shell = saved_text(&current, SHELL_FIELD, "auto");
    let shell_id = saved_text(&current, SHELL_ID_FIELD, "");
    let trusted_custom =
        !shell_id.is_empty() && config.local_shells.iter().any(|saved| saved.id == shell_id);

    let cwd = saved_text(&current, CWD_FIELD, "");
    let mut choice = LocalShellChoice {
        shell: shell.to_string(),
        ..LocalShellChoice::default()
    };

    match shell {
        "auto" | "cmd" | "powershell" | "pwsh" | "git-bash" => {}
        "custom" => {
            if !trusted_custom {
                return Err(
                    "The saved custom shell no longer exists in Settings, so its saved command \
                     was not run."
                        .to_string(),
                );
            }
            let path = saved_text(&current, SHELL_PATH_FIELD, "");
            if path.is_empty() || !valid_text(path) {
                return Err("The saved custom shell has no usable program path.".to_string());
            }
            choice.custom_path = Some(path.to_string());
            let args = saved_text(&current, SHELL_ARGS_FIELD, "");
            choice.custom_args = (!args.is_empty() && valid_text(args)).then(|| args.to_string());
        }
        "wsl" => {
            #[cfg(windows)]
            {
                let distro = saved_text(&current, SHELL_DISTRO_FIELD, "");
                if !distro.is_empty() && !valid_distro(distro) {
                    return Err("The saved WSL distribution name is not safe to start.".to_string());
                }
                choice.wsl_distro = (!distro.is_empty()).then(|| distro.to_string());
            }
            #[cfg(not(windows))]
            {
                return Err("WSL tabs can only be restored on Windows.".to_string());
            }
        }
        _ => {
            return Err(
                "The saved shell is not recognized, so the tab was kept without starting it."
                    .to_string(),
            );
        }
    }

    let mut options = local_options_for_choice(&choice)?;
    options.cwd = (!cwd.is_empty() && valid_text(cwd)).then(|| cwd.to_string());
    Ok(options)
}

/// The options a shell chosen in the launcher starts with. An explicit Auto
/// stays Auto; it is never replaced by the configured default.
pub(in crate::terminal_demo::window) fn local_options_for_choice(
    choice: &LocalShellChoice,
) -> Result<PtyConnectionOptions, String> {
    let shell = choice.shell.trim();
    let mut options = PtyConnectionOptions {
        connection_type: Some("terminal".to_string()),
        terminal_shell: Some(shell.to_string()),
        session_log: Some("never".to_string()),
        ..PtyConnectionOptions::default()
    };

    match shell {
        "auto" | "cmd" | "powershell" | "pwsh" | "git-bash" => {}
        "custom" => {
            let path = choice
                .custom_path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    "The selected custom shell has no usable program path.".to_string()
                })?;
            if !valid_text(path) {
                return Err(
                    "The selected custom shell's program path is not safe to start.".to_string(),
                );
            }
            options.terminal_shell_custom_path = Some(path.to_string());
            let args = choice
                .custom_args
                .as_deref()
                .map(str::trim)
                .filter(|args| !args.is_empty());
            if let Some(args) = args
                && !valid_text(args)
            {
                return Err(
                    "The selected custom shell's arguments are not safe to start.".to_string(),
                );
            }
            if let Some(args) = args {
                options.terminal_shell_custom_args = Some(args.to_string());
            }
        }
        "wsl" => {
            #[cfg(windows)]
            {
                let distro = choice
                    .wsl_distro
                    .as_deref()
                    .map(str::trim)
                    .filter(|distro| !distro.is_empty());
                if let Some(distro) = distro
                    && !valid_distro(distro)
                {
                    return Err(
                        "The selected WSL distribution name is not safe to start.".to_string()
                    );
                }
                if let Some(distro) = distro {
                    options.terminal_shell_wsl_distro = Some(distro.to_string());
                }
            }
            #[cfg(not(windows))]
            {
                return Err("WSL shells can only start on Windows.".to_string());
            }
        }
        _ => {
            return Err(format!(
                "The selected shell '{shell}' is not supported, so it was not started."
            ));
        }
    }
    Ok(options)
}

fn ssh_selector(tab: &Tab) -> Option<SshProfileSelector> {
    let connection = tab.connection.as_ref()?;
    if connection.connection_type != Some(ConnectionType::Ssh) {
        return None;
    }
    if let Some(id) = connection
        .saved_field("profileId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        return Some(SshProfileSelector::Id(id.to_string()));
    }
    connection
        .saved_field("profileName")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|name| SshProfileSelector::Name(name.to_string()))
}

fn saved_text<'a>(connection: &'a TabConnection, key: &str, fallback: &'a str) -> &'a str {
    match connection.saved_field(key) {
        None | Some(serde_json::Value::Null) => fallback,
        Some(serde_json::Value::String(value)) => value,
        Some(_) => fallback,
    }
}

fn valid_text(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

#[cfg(windows)]
fn valid_distro(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::{SshProfileSelector, local_options_for_choice, ssh_selector};
    use tterm_core::workspace::local_shells::types::LocalShellChoice;
    use tterm_core::workspace::tab::{ConnectionType, Tab, TabConnection, TabType};

    fn connection_tab(
        connection_type: ConnectionType,
        profile_id: Option<&str>,
        profile_name: Option<&str>,
    ) -> Tab {
        let mut connection = TabConnection::default();
        connection.set_connection_type(Some(connection_type));
        connection.set_profile_id(profile_id.map(str::to_string));
        connection.set_profile_name(profile_name.map(str::to_string));
        let mut tab = Tab::new("tab-1", "Saved", TabType::Ssh);
        tab.set_connection(Some(connection));
        tab
    }

    #[test]
    fn a_saved_ssh_tab_keeps_exact_ids_and_legacy_names_separate() {
        assert_eq!(
            ssh_selector(&connection_tab(
                ConnectionType::Ssh,
                Some("profile-1"),
                Some("Renamed"),
            )),
            Some(SshProfileSelector::Id("profile-1".to_string()))
        );
        assert_eq!(
            ssh_selector(&connection_tab(
                ConnectionType::Ssh,
                None,
                Some("Production"),
            )),
            Some(SshProfileSelector::Name("Production".to_string()))
        );
        assert_eq!(
            ssh_selector(&connection_tab(
                ConnectionType::Ssh,
                Some("  "),
                Some("Production"),
            )),
            Some(SshProfileSelector::Name("Production".to_string()))
        );
    }

    #[test]
    fn a_saved_tab_of_another_type_is_not_an_ssh_launch() {
        assert_eq!(
            ssh_selector(&connection_tab(
                ConnectionType::Terminal,
                Some("profile-1"),
                None,
            )),
            None
        );
    }

    #[test]
    fn a_selected_choice_keeps_its_shell_and_custom_values() {
        let custom = LocalShellChoice {
            shell: "custom".to_string(),
            custom_path: Some(" /bin/sh ".to_string()),
            custom_args: Some(" -l ".to_string()),
            ..LocalShellChoice::default()
        };
        let options = local_options_for_choice(&custom).expect("selected custom shell");
        assert_eq!(options.terminal_shell.as_deref(), Some("custom"));
        assert_eq!(
            options.terminal_shell_custom_path.as_deref(),
            Some("/bin/sh")
        );
        assert_eq!(options.terminal_shell_custom_args.as_deref(), Some("-l"));

        for shell in ["auto", "cmd", "powershell", "pwsh", "git-bash"] {
            let choice = LocalShellChoice {
                shell: shell.to_string(),
                ..LocalShellChoice::default()
            };
            assert_eq!(
                local_options_for_choice(&choice)
                    .expect("a supported shell")
                    .terminal_shell
                    .as_deref(),
                Some(shell)
            );
        }
    }

    #[test]
    fn an_unsupported_selected_shell_is_not_replaced_with_auto() {
        let unknown = LocalShellChoice {
            shell: "made-up".to_string(),
            ..LocalShellChoice::default()
        };
        assert!(local_options_for_choice(&unknown).is_err());

        let missing_path = LocalShellChoice {
            shell: "custom".to_string(),
            ..LocalShellChoice::default()
        };
        assert!(local_options_for_choice(&missing_path).is_err());
    }
}
