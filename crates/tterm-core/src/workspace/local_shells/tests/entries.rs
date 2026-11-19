use super::*;

use crate::workspace::local_shells::AUTO_SHELL_KEY;
use crate::workspace::local_shells::default_shell::default_shell_key;
use crate::workspace::local_shells::entries::{build_shell_entries, copied_shell_command};
use crate::workspace::local_shells::types::{
    CopiedShellCommand, LocalShellChoice, ShellConfig, ShellEntryKind,
};

#[test]
fn lists_auto_the_shells_found_here_then_the_users_own() {
    let shells = vec![nu(), msys()];
    let hidden = vec!["cmd".to_string()];
    let config = ShellConfig {
        local_shells: &shells,
        hidden_shells: &hidden,
        ..ShellConfig::default()
    };
    let entries = build_shell_entries(&windows_shells(), &config, translate);
    let listed: Vec<(&str, ShellEntryKind, bool)> = entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry.kind, entry.hidden))
        .collect();
    assert_eq!(
        listed,
        [
            (AUTO_SHELL_KEY, ShellEntryKind::Auto, false),
            ("cmd", ShellEntryKind::Detected, true),
            ("pwsh", ShellEntryKind::Detected, false),
            ("custom:nu", ShellEntryKind::Custom, false),
            ("custom:msys", ShellEntryKind::Custom, false),
        ]
    );
}

#[test]
fn the_users_shell_carries_its_path_arguments_and_id() {
    let shells = vec![nu(), msys()];
    let config = ShellConfig {
        local_shells: &shells,
        ..ShellConfig::default()
    };
    let entries = build_shell_entries(&windows_shells(), &config, translate);
    let msys_entry = &entries[4];
    assert_eq!(msys_entry.label, "MSYS2");
    assert_eq!(msys_entry.detail, r"C:\msys64\usr\bin\bash.exe --login -i");
    assert_eq!(
        msys_entry.choice,
        LocalShellChoice {
            shell: "custom".to_string(),
            custom_path: Some(r"C:\msys64\usr\bin\bash.exe".to_string()),
            custom_args: Some("--login -i".to_string()),
            shell_id: Some("msys".to_string()),
            wsl_distro: None,
        }
    );
}

#[test]
fn lists_each_wsl_distribution_under_its_own_key_and_wsls_default_while_that_is_the_default() {
    let wsl = r"C:\Windows\System32\wsl.exe";
    let distros = [
        TerminalShellProfile {
            default_wsl_distro: true,
            ..wsl_profile("Ubuntu", wsl, "Ubuntu")
        },
        wsl_profile("Debian", wsl, "Debian"),
    ];
    let keys = |config: &ShellConfig<'_>, profiles: &[TerminalShellProfile]| -> Vec<String> {
        build_shell_entries(profiles, config, translate)
            .into_iter()
            .map(|entry| entry.key)
            .collect()
    };
    assert_eq!(
        keys(&ShellConfig::default(), &distros),
        ["auto", "wsl:Ubuntu", "wsl:Debian"]
    );
    let wsl_default = ShellConfig {
        terminal_shell: "wsl",
        ..ShellConfig::default()
    };
    assert_eq!(
        keys(&wsl_default, &distros),
        ["auto", "wsl", "wsl:Ubuntu", "wsl:Debian"]
    );
    let listed = build_shell_entries(&distros, &wsl_default, translate);
    assert_eq!(listed[1].label, "Windows Subsystem for Linux (WSL)");
    assert_eq!(listed[1].choice, choice("wsl"));
    assert_eq!(default_shell_key(&wsl_default), "wsl");
    let unmarked = [
        wsl_profile("Ubuntu", wsl, "Ubuntu"),
        wsl_profile("Debian", wsl, "Debian"),
    ];
    assert_eq!(
        keys(&wsl_default, &unmarked),
        ["auto", "wsl", "wsl:Ubuntu", "wsl:Debian"]
    );
    let whole = [profile("wsl", "WSL", wsl)];
    assert_eq!(keys(&wsl_default, &whole), ["auto", "wsl"]);
    let plain = build_shell_entries(&distros, &ShellConfig::default(), translate);
    let debian = &plain[2];
    assert_eq!(debian.label, "Debian");
    assert_eq!(debian.detail, format!("{wsl} -d Debian"));
    assert_eq!(
        debian.choice,
        LocalShellChoice {
            shell: "wsl".to_string(),
            wsl_distro: Some("Debian".to_string()),
            ..LocalShellChoice::default()
        }
    );
    assert_eq!(
        copied_shell_command(debian),
        CopiedShellCommand {
            path: wsl.to_string(),
            args: "-d Debian".to_string(),
        }
    );
}

#[test]
fn keys_a_shell_found_on_path_by_its_path_and_shows_one_the_user_added_once() {
    let shells = vec![shell("f", "", "/opt/homebrew/bin/fish", "")];
    let config = ShellConfig {
        local_shells: &shells,
        ..ShellConfig::default()
    };
    let entries = build_shell_entries(&unix_shells(), &config, translate);
    let listed: Vec<(&str, &str)> = entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry.label.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("auto", "Auto (recommended)"),
            ("/bin/zsh", "zsh"),
            ("custom:f", "fish"),
        ]
    );
}

#[test]
fn lists_an_unknown_detected_shell_under_its_own_id() {
    let profiles = [profile("bogus", "Bogus shell", "/opt/bogus")];
    let entries = build_shell_entries(&profiles, &ShellConfig::default(), labelled);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].key, "bogus");
    assert_eq!(entries[1].choice.shell, "bogus");
    assert_eq!(entries[1].source, "/opt/bogus");
    assert_eq!(
        entries[1].label,
        "connection.terminalShellOptions.undefined|Bogus shell"
    );
}
