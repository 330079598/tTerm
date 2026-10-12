use super::*;

#[test]
fn installs_every_script_once_under_a_stable_version() {
    let script = install_script();
    for (path, content) in UNIX_SCRIPTS {
        assert_eq!(
            script.matches(&format!("\"$tterm_tmp/{path}\" <<")).count(),
            1,
            "{path}"
        );
        assert!(
            content.ends_with('\n') && !content.lines().any(|line| line == HEREDOC_END),
            "{path} must end with a line break and not end the heredoc early"
        );
    }
    assert!(!script.contains('\r') && !script.contains('@'));
    assert_eq!(version(), version());
    assert_eq!(version().len(), 12);
    assert!(script.contains(&format!("tterm_dir=$tterm_base/{}\n", version())));
    assert!(script.contains("-mtime +30 "));
}

#[test]
fn reads_the_answer_after_startup_noise() {
    assert_eq!(
        parse_reply("Welcome!\r\nTTERM-SHELL-INTEGRATION ok /home/me/.cache/t\r\n"),
        Some(Reply::Installed("/home/me/.cache/t"))
    );
    assert_eq!(
        parse_reply("TTERM-SHELL-INTEGRATION ok /home/my files/x\n"),
        Some(Reply::Installed("/home/my files/x"))
    );
    assert_eq!(
        parse_reply("TTERM-SHELL-INTEGRATION shell tcsh\n"),
        Some(Reply::Shell("tcsh"))
    );
    assert_eq!(
        parse_reply("TTERM-SHELL-INTEGRATION shell \n"),
        Some(Reply::Shell(""))
    );
    assert_eq!(
        parse_reply("TTERM-SHELL-INTEGRATION write-failed\n"),
        Some(Reply::WriteFailed)
    );
    assert_eq!(parse_reply(""), None);
    assert_eq!(parse_reply("TTERM-SHELL-INTEGRATION ok relative\n"), None);
    assert_eq!(parse_reply("sh: not found\n"), None);
}

#[test]
fn quotes_the_start_script_path() {
    assert_eq!(
        launch_command("/home/me/.cache/tterm/x").as_deref(),
        Some("exec sh '/home/me/.cache/tterm/x/unix/start.sh' --motd")
    );
    assert_eq!(
        launch_command("/home/my files/x").as_deref(),
        Some("exec sh '/home/my files/x/unix/start.sh' --motd")
    );
    assert_eq!(launch_command("/home/o'neil/x"), None);
    assert_eq!(launch_command("/home/me\n/x"), None);
}

#[test]
fn serializes_the_status_for_the_connection_header() {
    assert_eq!(
        serde_json::to_value(ShellIntegrationStatus::Active).unwrap(),
        serde_json::json!({ "status": "active" })
    );
    assert_eq!(
        serde_json::to_value(ShellIntegrationStatus::Unavailable {
            reason: UnavailableReason::Shell {
                shell: "tcsh".into()
            }
        })
        .unwrap(),
        serde_json::json!({
            "status": "unavailable",
            "reason": { "code": "shell", "shell": "tcsh" }
        })
    );
    assert_eq!(
        serde_json::to_value(UnavailableReason::NoReply).unwrap(),
        serde_json::json!({ "code": "noReply" })
    );
}
