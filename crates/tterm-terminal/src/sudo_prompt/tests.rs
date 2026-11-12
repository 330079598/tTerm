use super::{
    PasswordPromptMatch, compile_prompt_patterns, is_valid_prompt_pattern, match_password_prompt,
    read_cursor_line,
};
use crate::grid::{CellSize, GridSize};
use crate::terminal::{Config, Terminal};

const CELL: CellSize = CellSize::new(8, 16);

fn terminal(columns: usize, lines: usize) -> Terminal {
    Terminal::new(Config::default(), GridSize::new(columns, lines), CELL)
}

fn match_with(line: &str, user: Option<&str>) -> Option<PasswordPromptMatch> {
    match_password_prompt(line, user, &[])
}

#[test]
fn matches_the_default_english_sudo_prompt() {
    assert_eq!(
        match_with("[sudo] password for admin: ", Some("admin")),
        Some(PasswordPromptMatch {
            prompt: "[sudo] password for admin:".to_owned(),
            user: Some("admin".to_owned()),
        })
    );
}

#[test]
fn matches_the_quoted_user_name_of_newer_sudo_releases() {
    assert_eq!(
        match_with("[sudo] password for 'admin': ", Some("admin")).and_then(|found| found.user),
        Some("admin".to_owned())
    );
}

#[test]
fn matches_sudo_rs_prompts_which_do_not_name_the_user() {
    assert_eq!(
        match_with("[sudo: authenticate] Password: ", Some("stone")),
        Some(PasswordPromptMatch {
            prompt: "[sudo: authenticate] Password:".to_owned(),
            user: None,
        })
    );
    assert_eq!(
        match_with("[sudo: authenticate] 密码：", Some("stone")),
        Some(PasswordPromptMatch {
            prompt: "[sudo: authenticate] 密码：".to_owned(),
            user: None,
        })
    );
}

#[test]
fn ignores_sudo_rs_prompts_for_a_pin_or_one_time_code() {
    assert_eq!(
        match_with("[sudo: authenticate] PIN: ", Some("stone")),
        None
    );
    assert_eq!(
        match_with("[sudo: authenticate] Verification code: ", Some("stone")),
        None
    );
}

#[test]
fn matches_zh_cn_and_zh_tw_sudo_prompts_with_a_full_width_colon() {
    assert_eq!(
        match_with("[sudo] stone 的密码：", None).and_then(|found| found.user),
        Some("stone".to_owned())
    );
    assert_eq!(
        match_with("[sudo] stone 的密碼：", None).and_then(|found| found.user),
        Some("stone".to_owned())
    );
}

#[test]
fn matches_other_sudo_locales_when_the_login_user_appears_in_the_prompt() {
    assert_eq!(
        match_with("[sudo] Passwort für stone: ", Some("stone")),
        Some(PasswordPromptMatch {
            prompt: "[sudo] Passwort für stone:".to_owned(),
            user: Some("stone".to_owned()),
        })
    );
    assert_eq!(
        match_with("[sudo] mot de passe de stone : ", Some("stone")).and_then(|found| found.user),
        Some("stone".to_owned())
    );
}

#[test]
fn does_not_guess_the_user_of_an_unknown_sudo_locale() {
    assert_eq!(
        match_with("[sudo] Passwort für root: ", Some("stone")),
        None
    );
    assert_eq!(
        match_with("[sudo] Passwort für stoneage: ", Some("stone")),
        None
    );
}

#[test]
fn treats_an_empty_known_user_as_no_user_like_javascript() {
    assert_eq!(match_with("[sudo] mot de passe de root : ", Some("")), None);
    assert_eq!(match_with("[sudo] mot de passe de root : ", None), None);
    assert_eq!(
        match_with("[sudo] mot de passe de   root : ", Some(" "))
            .map(|found| (found.prompt, found.user)),
        Some((
            "[sudo] mot de passe de   root :".to_owned(),
            Some(" ".to_owned())
        ))
    );
}

#[test]
fn matches_doas_prompts() {
    assert_eq!(
        match_with("doas (ops@web-1) password: ", Some("ops")).and_then(|found| found.user),
        Some("ops".to_owned())
    );
}

#[test]
fn reports_the_requested_account_so_other_users_can_be_refused() {
    assert_eq!(
        match_with("[sudo] password for root: ", Some("stone")).and_then(|found| found.user),
        Some("root".to_owned())
    );
}

#[test]
fn ignores_ssh_logins_shell_prompts_and_log_lines() {
    assert_eq!(match_with("stone@db-2's password: ", Some("stone")), None);
    assert_eq!(match_with("stone@host:~$ ", Some("stone")), None);
    assert_eq!(match_with("my password for fun: 123", Some("stone")), None);
    assert_eq!(match_with("Password:", Some("stone")), None);
}

#[test]
fn applies_custom_patterns_with_an_optional_user_group() {
    let patterns = compile_prompt_patterns(&[
        "^Password for (?<user>\\S+) on \\S+:$".to_owned(),
        "^Password:$".to_owned(),
        "(".to_owned(),
    ]);
    assert_eq!(patterns.len(), 2);
    assert_eq!(
        match_password_prompt("Password for ops on db:", Some("ops"), &patterns)
            .and_then(|found| found.user),
        Some("ops".to_owned())
    );
    assert_eq!(
        match_password_prompt("Password:", Some("ops"), &patterns),
        Some(PasswordPromptMatch {
            prompt: "Password:".to_owned(),
            user: None,
        })
    );
}

#[test]
fn requires_every_prompt_to_end_with_a_colon_custom_ones_included() {
    let patterns = compile_prompt_patterns(&["^Enter PIN>$".to_owned()]);
    assert_eq!(
        match_password_prompt("Enter PIN>", Some("ops"), &patterns),
        None
    );
}

#[test]
fn rejects_malformed_regular_expressions() {
    assert!(is_valid_prompt_pattern("^Password:$"));
    assert!(!is_valid_prompt_pattern("("));
    assert!(is_valid_prompt_pattern(""));
    assert!(!is_valid_prompt_pattern("\\"));
}

#[test]
fn compiles_custom_patterns_after_javascript_trimming() {
    let patterns = compile_prompt_patterns(&[
        "  ^Password:$  ".to_owned(),
        "\u{3000}^PIN:$\u{3000}".to_owned(),
        "   ".to_owned(),
        "(".to_owned(),
    ]);
    assert_eq!(patterns.len(), 2);
    assert_eq!(
        match_password_prompt("Password:", None, &patterns).map(|found| found.prompt),
        Some("Password:".to_owned())
    );
}

#[test]
fn sudo_rs_password_words_are_case_insensitive() {
    for line in [
        "[sudo: authenticate] PASSWORD:",
        "[sudo: authenticate] Passwort:",
        "[sudo: authenticate] contraseña:",
        "[sudo: authenticate] ПАРОЛЬ：",
    ] {
        assert_eq!(
            match_with(line, Some("stone")),
            Some(PasswordPromptMatch {
                prompt: line.to_owned(),
                user: None,
            }),
            "{line}"
        );
    }
}

#[test]
fn a_long_line_is_not_a_prompt() {
    let accepted = format!("[sudo] {} 的密码：", "a".repeat(244));
    let rejected = format!("[sudo] {} 的密码：", "a".repeat(245));
    assert!(match_with(&accepted, None).is_some());
    assert_eq!(match_with(&rejected, None), None);
}

#[test]
fn escapes_the_known_user_in_the_localized_prompt_pattern() {
    assert_eq!(
        match_with("[sudo] Password für a.b: ", Some("a.b")).and_then(|found| found.user),
        Some("a.b".to_owned())
    );
    assert_eq!(match_with("[sudo] Password für a.b: ", Some("stone")), None);
}

#[test]
fn counts_the_prompt_limit_in_utf16_units() {
    let accepted = format!("[sudo] {} 的密码：", "😀".repeat(122));
    let user = "😀".repeat(122);
    assert_eq!(
        match_with(&accepted, Some("stone")),
        Some(PasswordPromptMatch {
            prompt: accepted.clone(),
            user: Some(user),
        })
    );
    let rejected = format!("[sudo] {} 的密码：", "😀".repeat(123));
    assert_eq!(match_with(&rejected, Some("stone")), None);
}

#[test]
fn reads_the_visible_text_before_the_cursor_ignoring_colors_and_redraws() {
    let terminal = terminal(80, 10);
    terminal.write(b"$ sudo ls\r\n\x1b[1m[sudo]\x1b[0m password for ");
    terminal.write(b"stone: \x1b7\x1b[10;1Hstatus\x1b8");
    assert_eq!(
        read_cursor_line(&terminal.snapshot()).trim(),
        "[sudo] password for stone:"
    );
}

#[test]
fn joins_a_prompt_that_wrapped_across_rows() {
    let terminal = terminal(12, 10);
    terminal.write(b"[sudo] password for stone: ");
    assert_eq!(
        read_cursor_line(&terminal.snapshot()).trim(),
        "[sudo] password for stone:"
    );
}
