//! Tests for the config-file local-shell writes.

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

fn listed(id: &str, path: &str) -> LocalShell {
    LocalShell {
        id: id.to_string(),
        name: id.to_string(),
        path: path.to_string(),
        args: String::new(),
    }
}

fn own_choice(path: &str, args: &str) -> LocalShellChoice {
    LocalShellChoice {
        shell: "custom".to_string(),
        custom_path: Some(path.to_string()),
        custom_args: (!args.is_empty()).then(|| args.to_string()),
        shell_id: Some(SHELL_ID.to_string()),
        wsl_distro: None,
    }
}

fn seed(path: &std::path::Path, config: AppConfig) {
    super::super::compatible_file::save_at(path, &config).unwrap();
}

fn hidden(path: &std::path::Path) -> Vec<String> {
    super::super::load_config_at(path).unwrap().hidden_shells
}

fn visibility(key: &str, hidden: bool) -> ShellListEdit {
    ShellListEdit::Visibility {
        key: key.to_string(),
        hidden,
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
    seed(&path, seeded);
    let extended = fs::read_to_string(&path)
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
    seed(
        &path,
        AppConfig {
            local_shells: vec![first, second.clone()],
            ..AppConfig::default()
        },
    );

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
    seed(
        &path,
        AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_id: SHELL_ID.to_string(),
            terminal_shell_custom_path: "/bin/nu".to_string(),
            local_shells: vec![shell("/bin/nu", "")],
            ..AppConfig::default()
        },
    );

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

#[test]
fn saves_a_default_against_the_latest_shell_and_preserves_unknown_fields() {
    let path = temp_config();
    let latest = shell("/opt/homebrew/bin/nu", "-l");
    seed(
        &path,
        AppConfig {
            font_size: 24,
            local_shells: vec![latest.clone()],
            ..AppConfig::default()
        },
    );
    let extended =
        fs::read_to_string(&path)
            .unwrap()
            .replacen('{', r#"{"future":{"keep":[1,2]},"#, 1);
    fs::write(&path, extended).unwrap();

    let saved = save_default_at(&path, own_choice("/stale/nu", "--stale")).unwrap();

    assert_eq!(
        saved.default_shell,
        default_shell_updates(&own_shell_choice(&latest))
    );
    assert_eq!(saved.local_shells, vec![latest]);
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(reloaded.font_size, 24);
    assert_eq!(default_shell_settings(&reloaded), saved.default_shell);
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains(r#""keep":[1,2]"#)
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_a_default_shell_removed_by_another_edit() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            local_shells: vec![shell("/bin/nu", "")],
            ..AppConfig::default()
        },
    );
    let before = fs::read_to_string(&path).unwrap();

    let result = save_default_at(
        &path,
        LocalShellChoice {
            shell_id: Some("removed-shell".to_string()),
            ..own_choice("/bin/nu", "")
        },
    );

    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    fs::remove_file(path).unwrap();
}

#[test]
fn resets_the_default_shell_without_dropping_the_saved_list() {
    let path = temp_config();
    let saved_shell = shell("/bin/nu", "-l");
    seed(
        &path,
        AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: "/bin/nu".to_string(),
            terminal_shell_custom_args: "-l".to_string(),
            terminal_shell_id: SHELL_ID.to_string(),
            local_shells: vec![saved_shell.clone()],
            ..AppConfig::default()
        },
    );

    let saved = save_default_at(&path, LocalShellChoice::default()).unwrap();

    assert_eq!(
        saved.default_shell,
        default_shell_updates(&LocalShellChoice::default())
    );
    assert_eq!(saved.local_shells, vec![saved_shell]);
    assert_eq!(
        default_shell_settings(&super::super::load_config_at(&path).unwrap()),
        saved.default_shell
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_to_overwrite_an_unreadable_config_for_a_default_change() {
    let path = temp_config();
    for damaged in ["{", r#"{"theme":false}"#] {
        fs::write(&path, damaged).unwrap();
        assert!(save_default_at(&path, LocalShellChoice::default()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn hides_and_shows_a_detected_shell_without_dropping_unrelated_settings() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            font_size: 24,
            hidden_shells: vec!["cmd".to_string()],
            local_shells: vec![listed("nu", "/bin/nu")],
            ..AppConfig::default()
        },
    );
    let extended =
        fs::read_to_string(&path)
            .unwrap()
            .replacen('{', r#"{"future":{"keep":[1,2]},"#, 1);
    fs::write(&path, extended).unwrap();

    let saved = edit_shell_list_at(&path, visibility("pwsh", true)).unwrap();
    assert_eq!(saved.hidden_shells, ["cmd", "pwsh"]);
    assert_eq!(saved.local_shells, [listed("nu", "/bin/nu")]);
    assert_eq!(
        saved,
        SavedShellLists::from(&super::super::load_config_at(&path).unwrap())
    );

    let again = edit_shell_list_at(&path, visibility("pwsh", true)).unwrap();
    assert_eq!(again.hidden_shells, ["cmd", "pwsh"]);

    let shown = edit_shell_list_at(&path, visibility("cmd", false)).unwrap();
    assert_eq!(shown.hidden_shells, ["pwsh"]);
    let shown_again = edit_shell_list_at(&path, visibility("cmd", false)).unwrap();
    assert_eq!(shown_again.hidden_shells, ["pwsh"]);

    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(reloaded.font_size, 24);
    assert_eq!(reloaded.local_shells, [listed("nu", "/bin/nu")]);
    assert_eq!(reloaded.hidden_shells, ["pwsh"]);
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains(r#""keep":[1,2]"#)
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_to_hide_auto_an_added_shell_or_the_default() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            terminal_shell: "pwsh".to_string(),
            local_shells: vec![listed("nu", "/bin/nu")],
            ..AppConfig::default()
        },
    );

    for key in ["auto", "custom:nu", "pwsh"] {
        let before = fs::read_to_string(&path).unwrap();
        assert!(
            edit_shell_list_at(&path, visibility(key, true)).is_err(),
            "{key} must not be hidden"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    let empty = edit_shell_list_at(&path, visibility("", true));
    assert!(empty.is_err());

    let shown = edit_shell_list_at(&path, visibility("auto", false)).unwrap();
    assert_eq!(shown.hidden_shells, Vec::<String>::new());
    fs::remove_file(path).unwrap();
}

#[test]
fn treats_hiding_an_already_hidden_shell_as_a_no_op() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            terminal_shell: "pwsh".to_string(),
            hidden_shells: vec!["pwsh".to_string()],
            ..AppConfig::default()
        },
    );

    let saved = edit_shell_list_at(&path, visibility("pwsh", true)).unwrap();

    assert_eq!(saved.hidden_shells, ["pwsh"]);
    fs::remove_file(path).unwrap();
}

#[test]
fn moves_a_listed_shell_by_one_place_and_validates_the_offset() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            local_shells: vec![listed("zsh", "/bin/zsh"), listed("nu", "/bin/nu")],
            ..AppConfig::default()
        },
    );

    let moved = edit_shell_list_at(
        &path,
        ShellListEdit::Move {
            id: "nu".to_string(),
            offset: -1,
        },
    )
    .unwrap();
    assert_eq!(
        moved.local_shells,
        [listed("nu", "/bin/nu"), listed("zsh", "/bin/zsh")]
    );

    let moved = edit_shell_list_at(
        &path,
        ShellListEdit::Move {
            id: "nu".to_string(),
            offset: 1,
        },
    )
    .unwrap();
    assert_eq!(
        moved.local_shells,
        [listed("zsh", "/bin/zsh"), listed("nu", "/bin/nu")]
    );

    for offset in [0, 2, -2] {
        let before = fs::read_to_string(&path).unwrap();
        let result = edit_shell_list_at(
            &path,
            ShellListEdit::Move {
                id: "nu".to_string(),
                offset,
            },
        );
        assert!(result.is_err(), "offset {offset} must be rejected");
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
    }

    let missing = edit_shell_list_at(
        &path,
        ShellListEdit::Move {
            id: "missing".to_string(),
            offset: -1,
        },
    )
    .unwrap();
    assert_eq!(
        missing.local_shells,
        [listed("zsh", "/bin/zsh"), listed("nu", "/bin/nu")]
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn removes_a_shell_and_falls_back_to_auto_when_it_was_the_default() {
    let path = temp_config();
    seed(
        &path,
        AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_custom_path: "/bin/nu".to_string(),
            terminal_shell_custom_args: "-l".to_string(),
            terminal_shell_id: SHELL_ID.to_string(),
            hidden_shells: vec!["cmd".to_string()],
            local_shells: vec![shell("/bin/nu", "-l"), listed("fish", "/bin/fish")],
            ..AppConfig::default()
        },
    );

    let saved = edit_shell_list_at(
        &path,
        ShellListEdit::Remove {
            id: SHELL_ID.to_string(),
        },
    )
    .unwrap();

    assert_eq!(saved.local_shells, [listed("fish", "/bin/fish")]);
    assert_eq!(
        saved.default_shell,
        default_shell_updates(&LocalShellChoice::default())
    );
    assert_eq!(saved.hidden_shells, ["cmd"]);
    let reloaded = super::super::load_config_at(&path).unwrap();
    assert_eq!(reloaded.local_shells, saved.local_shells);
    assert_eq!(reloaded.hidden_shells, ["cmd"]);
    assert_eq!(default_shell_settings(&reloaded), saved.default_shell);

    let missing = edit_shell_list_at(
        &path,
        ShellListEdit::Remove {
            id: "missing".to_string(),
        },
    )
    .unwrap();
    assert_eq!(missing.local_shells, saved.local_shells);
    fs::remove_file(path).unwrap();
}

#[test]
fn keeps_the_hidden_list_through_unrelated_shell_saves() {
    let path = temp_config();
    let saved_shell = shell("/bin/nu", "-l");
    seed(
        &path,
        AppConfig {
            local_shells: vec![saved_shell.clone()],
            ..AppConfig::default()
        },
    );
    edit_shell_list_at(&path, visibility("pwsh", true)).unwrap();

    save_at(&path, shell("/opt/homebrew/bin/nu", "-l")).unwrap();
    assert_eq!(hidden(&path), ["pwsh"]);

    save_default_at(&path, own_choice("/opt/homebrew/bin/nu", "-l")).unwrap();
    assert_eq!(hidden(&path), ["pwsh"]);
    assert_eq!(
        super::super::load_config_at(&path).unwrap().local_shells,
        [shell("/opt/homebrew/bin/nu", "-l")]
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn refuses_to_overwrite_an_unreadable_config_for_a_list_edit() {
    let path = temp_config();
    for damaged in ["{", r#"{"theme":false}"#] {
        fs::write(&path, damaged).unwrap();
        assert!(edit_shell_list_at(&path, visibility("pwsh", true)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), damaged);
    }
    fs::remove_file(path).unwrap();
}
