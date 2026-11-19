use super::*;

use crate::workspace::local_shells::shell_label::{
    is_detected_custom_shell, shell_label_key, terminal_shell_label,
};
use crate::workspace::tab::TerminalShellType;

fn label(shell: &str, source: &str, distro: Option<&str>) -> String {
    let mut profile = profile(shell, &format!("{shell} label"), source);
    profile.wsl_distro = distro.map(str::to_string);
    terminal_shell_label(&profile, labelled)
}

#[test]
fn translates_the_shells_the_app_picks_itself() {
    assert_eq!(
        label("auto", "src", None),
        "connection.terminalShellOptions.auto|auto label"
    );
    assert_eq!(
        label("cmd", "src", None),
        "connection.terminalShellOptions.cmd|cmd label"
    );
    assert_eq!(
        label("powershell", "src", None),
        "connection.terminalShellOptions.powershell|powershell label"
    );
    assert_eq!(
        label("pwsh", "src", None),
        "connection.terminalShellOptions.pwsh|pwsh label"
    );
    assert_eq!(
        label("wsl", "src", None),
        "connection.terminalShellOptions.wsl|wsl label"
    );
    assert_eq!(
        label("git-bash", "src", None),
        "connection.terminalShellOptions.gitBash|git-bash label"
    );
    assert_eq!(
        label("custom", "manual path", None),
        "connection.terminalShellOptions.custom|custom label"
    );
}

#[test]
fn keeps_the_label_of_a_shell_found_on_this_machine_or_a_wsl_distribution() {
    assert_eq!(label("custom", "/bin/zsh", None), "custom label");
    assert_eq!(label("wsl", "src", Some("Ubuntu")), "wsl label");
    assert_eq!(
        label("bogus", "src", None),
        "connection.terminalShellOptions.undefined|bogus label"
    );
}

#[test]
fn knows_a_custom_shell_found_on_this_machine() {
    assert!(!is_detected_custom_shell(&profile(
        "custom",
        "Custom executable",
        "manual path"
    )));
    assert!(!is_detected_custom_shell(&profile(
        "custom",
        "Default shell",
        "$SHELL"
    )));
    assert!(is_detected_custom_shell(&profile(
        "custom", "zsh", "/bin/zsh"
    )));
    assert!(!is_detected_custom_shell(&profile(
        "pwsh",
        "PowerShell 7 (pwsh)",
        "x"
    )));
}

#[test]
fn names_the_catalogue_key_of_every_shell_kind() {
    let key = |shell| shell_label_key(shell);
    assert_eq!(
        key(TerminalShellType::Auto),
        "connection.terminalShellOptions.auto"
    );
    assert_eq!(
        key(TerminalShellType::Cmd),
        "connection.terminalShellOptions.cmd"
    );
    assert_eq!(
        key(TerminalShellType::Powershell),
        "connection.terminalShellOptions.powershell"
    );
    assert_eq!(
        key(TerminalShellType::Pwsh),
        "connection.terminalShellOptions.pwsh"
    );
    assert_eq!(
        key(TerminalShellType::Wsl),
        "connection.terminalShellOptions.wsl"
    );
    assert_eq!(
        key(TerminalShellType::GitBash),
        "connection.terminalShellOptions.gitBash"
    );
    assert_eq!(
        key(TerminalShellType::Custom),
        "connection.terminalShellOptions.custom"
    );
}
