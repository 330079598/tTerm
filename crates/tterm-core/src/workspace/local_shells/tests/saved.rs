use super::*;

use crate::workspace::local_shells::connections::local_shell_connection;
use crate::workspace::local_shells::saved::{
    failed_shell_fallback, moved_local_shell, new_local_shell_id, own_shell_choice,
    removed_local_shell, saved_local_shell, saved_local_shell_updates, toggled_hidden_shell,
};
use crate::workspace::local_shells::types::{
    DefaultShellChange, DefaultShellSettings, FailedShellFallback, LocalShellChoice, ShellConfig,
    ShellListUpdate,
};

fn auto_settings() -> DefaultShellSettings {
    DefaultShellSettings {
        terminal_shell: "auto".to_string(),
        ..DefaultShellSettings::default()
    }
}

fn custom_default<'a>(path: &'a str, shells: &'a [LocalShell]) -> ShellConfig<'a> {
    ShellConfig {
        terminal_shell: "custom",
        terminal_shell_custom_path: path,
        terminal_shell_id: "nu",
        local_shells: shells,
        ..ShellConfig::default()
    }
}

#[test]
fn keep_the_defaults_path_in_step_when_the_default_is_edited() {
    let listed = nu();
    let shells = vec![listed.clone()];
    let config = custom_default(&listed.path, &shells);
    let edited = shell("nu", "nu", r"D:\nu.exe", "-l");
    assert_eq!(
        saved_local_shell(&config, &edited),
        ShellListUpdate {
            local_shells: vec![edited.clone()],
            default_shell: Some(DefaultShellChange::Edited {
                path: r"D:\nu.exe".to_string(),
                args: "-l".to_string(),
            }),
        }
    );
    let plain = ShellConfig {
        local_shells: &shells,
        ..ShellConfig::default()
    };
    assert_eq!(
        saved_local_shell(&plain, &msys()),
        ShellListUpdate {
            local_shells: vec![listed, msys()],
            default_shell: None,
        }
    );
}

#[test]
fn leave_auto_as_the_default_when_the_default_is_removed() {
    let shells = vec![nu(), msys()];
    let config = ShellConfig {
        terminal_shell: "custom",
        terminal_shell_id: "nu",
        local_shells: &shells,
        ..ShellConfig::default()
    };
    assert_eq!(
        removed_local_shell(&config, "nu"),
        ShellListUpdate {
            local_shells: vec![msys()],
            default_shell: Some(DefaultShellChange::Chosen(auto_settings())),
        }
    );
    assert_eq!(
        removed_local_shell(&config, "msys"),
        ShellListUpdate {
            local_shells: vec![nu()],
            default_shell: None,
        }
    );
}

#[test]
fn move_one_place_at_a_time_and_stop_at_either_end() {
    let shells = vec![nu(), msys()];
    assert_eq!(moved_local_shell(&shells, "msys", -1), [msys(), nu()]);
    assert_eq!(moved_local_shell(&shells, "nu", -1), shells);
    assert_eq!(moved_local_shell(&shells, "missing", 1), shells);
    assert_eq!(moved_local_shell(&shells, "nu", 1), [msys(), nu()]);
}

#[test]
fn hide_and_show_detected_shells_by_key() {
    let hidden = vec!["cmd".to_string()];
    assert_eq!(toggled_hidden_shell(&hidden, "pwsh"), ["cmd", "pwsh"]);
    let both = vec!["cmd".to_string(), "pwsh".to_string()];
    assert_eq!(toggled_hidden_shell(&both, "cmd"), ["pwsh"]);
}

#[test]
fn offers_the_default_shell_or_auto_when_the_default_is_the_one_that_failed() {
    let listed = nu();
    let shells = vec![listed.clone()];
    let config = custom_default(&listed.path, &shells);
    let msys_connection = local_shell_connection(&own_shell_choice(&msys()), None);
    assert_eq!(
        failed_shell_fallback(Some(&msys_connection), &config),
        Some(FailedShellFallback {
            choice: own_shell_choice(&listed),
            replaces_default: false,
        })
    );
    let nu_connection = local_shell_connection(&own_shell_choice(&listed), None);
    assert_eq!(
        failed_shell_fallback(Some(&nu_connection), &config),
        Some(FailedShellFallback {
            choice: LocalShellChoice::default(),
            replaces_default: true,
        })
    );
    assert_eq!(
        failed_shell_fallback(Some(&msys_connection), &ShellConfig::default()),
        Some(FailedShellFallback {
            choice: LocalShellChoice::default(),
            replaces_default: false,
        })
    );
    let auto_connection = local_shell_connection(&LocalShellChoice::default(), None);
    assert_eq!(
        failed_shell_fallback(Some(&auto_connection), &ShellConfig::default()),
        None
    );
}

#[test]
fn saves_a_shell_and_makes_it_the_default_in_one_change() {
    assert_eq!(
        saved_local_shell_updates(&ShellConfig::default(), &nu(), true),
        ShellListUpdate {
            local_shells: vec![nu()],
            default_shell: Some(DefaultShellChange::Chosen(DefaultShellSettings {
                terminal_shell: "custom".to_string(),
                terminal_shell_custom_path: nu().path,
                terminal_shell_custom_args: String::new(),
                terminal_shell_id: "nu".to_string(),
                terminal_shell_wsl_distro: String::new(),
            })),
        }
    );
    assert_eq!(
        saved_local_shell_updates(&ShellConfig::default(), &nu(), false),
        ShellListUpdate {
            local_shells: vec![nu()],
            default_shell: None,
        }
    );
}

#[test]
fn gives_a_new_shell_a_fresh_uuid() {
    let id = new_local_shell_id();
    assert_eq!(id.len(), 36);
    assert!(
        id.chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
    );
    assert!(uuid::Uuid::parse_str(&id).is_ok());
}
