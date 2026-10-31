use super::command::{script_shell, sftp_server, MISSING_SFTP_SERVER, READY_MARKER};
use super::errors::{elevation_failed, is_wrong_password, probe_failed, validation_failed};
use super::SudoAuth;
use zeroize::Zeroizing;

fn password() -> SudoAuth {
    SudoAuth::Password(Zeroizing::new("secret".to_string()))
}

/// The script inside the single quotes of a command run through the login
/// shell.
fn quoted_script(command: &str) -> &str {
    let start = command.find("sh -c '").expect("runs sh -c") + "sh -c '".len();
    assert!(command.ends_with('\''), "{command}");
    &command[start..command.len() - 1]
}

#[test]
fn commands_read_the_same_in_every_login_shell() {
    for command in [
        sftp_server(&SudoAuth::NoPassword),
        sftp_server(&password()),
        script_shell(&SudoAuth::NoPassword),
        script_shell(&password()),
    ] {
        let script = quoted_script(&command);
        for unsafe_text in ["'", "!", "\\\\", "\n"] {
            assert!(
                !script.contains(unsafe_text),
                "{unsafe_text:?} in {command}"
            );
        }
        assert!(script.contains(READY_MARKER), "{command}");
    }
}

#[test]
fn sudo_never_reuses_cached_credentials() {
    assert!(sftp_server(&SudoAuth::NoPassword).starts_with("sudo -k -n -- sh -c '"));
    assert!(sftp_server(&password()).starts_with("sudo -k -S -p '' -- sh -c '"));
    assert!(script_shell(&password()).starts_with("sudo -k -S -p '' -- sh -c '"));
}

#[test]
fn the_sftp_server_script_tries_sshd_config_first_and_names_a_missing_program() {
    let command = sftp_server(&SudoAuth::NoPassword);
    let script = quoted_script(&command);
    let config = script
        .find("/etc/ssh/sshd_config")
        .expect("reads sshd_config");
    let debian = script
        .find("/usr/lib/openssh/sftp-server")
        .expect("tries the Debian path");
    assert!(config < debian);
    assert!(script.ends_with(&format!("echo \"{MISSING_SFTP_SERVER}\" >&2; exit 127")));
}

#[test]
fn a_retry_prompt_means_a_wrong_password_in_both_sudos() {
    assert!(is_wrong_password("Sorry, try again.\n"));
    assert!(is_wrong_password(
        "sudo: Authentication failed, try again.\n"
    ));
    assert!(!is_wrong_password("sudo: a password is required\n"));
}

#[test]
fn the_probe_stops_only_for_a_tty_or_a_missing_sudo() {
    let tty = probe_failed(Some(1), "sudo: sorry, you must have a tty to run sudo\n");
    assert!(tty.expect("tty").contains("requiretty"));
    let missing = probe_failed(Some(127), "bash: line 1: sudo: command not found\n");
    assert!(missing.expect("missing").contains("not installed"));

    // Wanting a password, or a refusal sudo-rs gives before asking for one,
    // goes on to the password check, which says why.
    for stderr in [
        "sudo: a password is required\n",
        "sudo: interactive authentication is required\n",
        "sudo: I'm sorry dave. I'm afraid I can't do that\n",
    ] {
        assert_eq!(probe_failed(Some(1), stderr), None, "{stderr}");
    }
}

#[test]
fn a_refused_password_check_keeps_the_words_of_either_sudo() {
    let wrong =
        "Sorry, try again.\nsudo: no password was provided\nsudo: 1 incorrect password attempt\n";
    assert!(validation_failed(Some(1), wrong).contains("did not accept"));
    let wrong_rs = "\nsudo: Authentication failed, try again.\n\nsudo: Authentication required but not attempted\n";
    assert!(validation_failed(Some(1), wrong_rs).contains("did not accept"));

    let classic = validation_failed(Some(1), "Sorry, user dave may not run sudo on web01.\n");
    assert!(classic.contains("may not run sudo on web01"), "{classic}");
    let rs = validation_failed(
        Some(1),
        "sudo: I'm sorry dave. I'm afraid I can't do that\n",
    );
    assert!(rs.contains("I'm afraid I can't do that"), "{rs}");
    assert!(validation_failed(Some(1), "").contains("exit status 1"));
}

#[test]
fn a_channel_that_never_got_ready_says_why() {
    let missing = elevation_failed(Some(127), &format!("{MISSING_SFTP_SERVER}\n"));
    assert!(missing.contains("internal-sftp"), "{missing}");

    // sudo 1.9 quotes the refused command, script and all, so the missing
    // program's line appears inside its refusal.
    let refusal = format!(
        "Sorry, user carol is not allowed to execute '/usr/bin/sh -c {}' as root on web01.\n",
        quoted_script(&sftp_server(&password()))
    );
    let restricted = elevation_failed(Some(1), &refusal);
    assert!(restricted.contains("only some commands"), "{restricted}");
    assert!(restricted.contains("carol is not allowed"), "{restricted}");
}
