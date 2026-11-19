use super::*;

use crate::workspace::local_shells::default_shell::{
    default_shell_choice, default_shell_key, default_shell_updates, shell_choice_key,
};
use crate::workspace::local_shells::saved::own_shell_choice;
use crate::workspace::local_shells::types::{DefaultShellSettings, LocalShellChoice, ShellConfig};

fn debian() -> LocalShellChoice {
    LocalShellChoice {
        shell: "wsl".to_string(),
        wsl_distro: Some("Debian".to_string()),
        ..LocalShellChoice::default()
    }
}

#[test]
fn is_found_by_the_id_of_the_users_shell_or_else_by_its_kind_or_path() {
    assert_eq!(default_shell_key(&ShellConfig::default()), "auto");
    assert_eq!(
        default_shell_key(&ShellConfig {
            terminal_shell: "pwsh",
            ..ShellConfig::default()
        }),
        "pwsh"
    );
    assert_eq!(
        default_shell_key(&ShellConfig {
            terminal_shell: "custom",
            terminal_shell_custom_path: "/bin/zsh",
            ..ShellConfig::default()
        }),
        "/bin/zsh"
    );
    let shells = vec![nu()];
    assert_eq!(
        default_shell_key(&ShellConfig {
            terminal_shell: "custom",
            terminal_shell_custom_path: "old",
            terminal_shell_id: "nu",
            local_shells: &shells,
            ..ShellConfig::default()
        }),
        "custom:nu"
    );
}

#[test]
fn starts_with_the_current_path_of_the_users_shell() {
    let listed = msys();
    let shells = vec![listed.clone()];
    let config = ShellConfig {
        terminal_shell: "custom",
        terminal_shell_custom_path: "stale.exe",
        terminal_shell_id: "msys",
        local_shells: &shells,
        ..ShellConfig::default()
    };
    assert_eq!(
        default_shell_choice(&config),
        LocalShellChoice {
            shell: "custom".to_string(),
            custom_path: Some(listed.path.clone()),
            custom_args: Some("--login -i".to_string()),
            shell_id: Some("msys".to_string()),
            wsl_distro: None,
        }
    );
    assert_eq!(
        default_shell_choice(&ShellConfig {
            terminal_shell: "cmd",
            ..ShellConfig::default()
        }),
        choice("cmd")
    );
}

#[test]
fn is_set_with_the_path_older_versions_read() {
    assert_eq!(
        default_shell_updates(&LocalShellChoice {
            shell: "custom".to_string(),
            custom_path: Some(" nu ".to_string()),
            custom_args: Some("-l".to_string()),
            shell_id: Some("nu".to_string()),
            wsl_distro: None,
        }),
        DefaultShellSettings {
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: "nu".to_string(),
            terminal_shell_custom_args: "-l".to_string(),
            terminal_shell_id: "nu".to_string(),
            terminal_shell_wsl_distro: String::new(),
        }
    );
    let builtin = LocalShellChoice {
        custom_path: Some("x".to_string()),
        ..choice("pwsh")
    };
    assert_eq!(
        default_shell_updates(&builtin),
        DefaultShellSettings {
            terminal_shell: "pwsh".to_string(),
            terminal_shell_custom_path: String::new(),
            terminal_shell_custom_args: String::new(),
            terminal_shell_id: String::new(),
            terminal_shell_wsl_distro: String::new(),
        }
    );
}

#[test]
fn can_be_a_wsl_distribution_or_wsls_own_default() {
    let debian_updates = default_shell_updates(&debian());
    assert_eq!(debian_updates.terminal_shell_wsl_distro, "Debian");
    let base = ShellConfig::default();
    let config = base.with_default(&debian_updates);
    assert_eq!(default_shell_key(&config), "wsl:Debian");
    assert_eq!(default_shell_choice(&config), debian());
    let wsl_default = ShellConfig {
        terminal_shell: "wsl",
        ..ShellConfig::default()
    };
    assert_eq!(default_shell_key(&wsl_default), "wsl");
    assert_eq!(default_shell_choice(&wsl_default), choice("wsl"));
}

#[test]
fn names_the_entry_a_choice_would_have_as_the_default() {
    let shells = vec![nu()];
    let config = ShellConfig {
        local_shells: &shells,
        ..ShellConfig::default()
    };
    let own = own_shell_choice(&nu());
    assert_eq!(shell_choice_key(&own, &config), "custom:nu");
    assert_eq!(shell_choice_key(&debian(), &config), "wsl:Debian");
}

#[test]
fn keeps_an_unknown_default_shell_id() {
    let config = ShellConfig {
        terminal_shell: "bogus",
        ..ShellConfig::default()
    };
    assert_eq!(default_shell_key(&config), "bogus");
    let chosen = default_shell_choice(&config);
    assert_eq!(chosen.shell, "bogus");
    assert_eq!(chosen.custom_path, None);
    assert_eq!(chosen.custom_args, None);
    assert_eq!(chosen.wsl_distro, None);
    assert_eq!(default_shell_updates(&chosen).terminal_shell, "bogus");
    assert_eq!(shell_choice_key(&chosen, &ShellConfig::default()), "bogus");
}

#[test]
fn keeps_missing_choice_fields_missing() {
    let spaced = ShellConfig {
        terminal_shell: "wsl",
        terminal_shell_wsl_distro: "  ",
        ..ShellConfig::default()
    };
    assert_eq!(default_shell_choice(&spaced).wsl_distro, None);
    assert_eq!(default_shell_key(&spaced), "wsl");
    let custom = ShellConfig {
        terminal_shell: "custom",
        terminal_shell_custom_path: " /bin/zsh ",
        ..ShellConfig::default()
    };
    let chosen = default_shell_choice(&custom);
    assert_eq!(chosen.custom_path.as_deref(), Some("/bin/zsh"));
    assert_eq!(chosen.custom_args, None);
    let updates = default_shell_updates(&LocalShellChoice::default());
    assert_eq!(updates.terminal_shell_custom_args, "");
    assert_eq!(updates.terminal_shell_wsl_distro, "");
}
