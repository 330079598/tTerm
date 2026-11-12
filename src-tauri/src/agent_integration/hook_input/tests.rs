use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn run(input: &str) -> String {
    run_command(
        &format!("{}; printf '%s;%s' \"$sid\" \"$cwd\"", script()),
        input,
    )
}

fn run_command(command: &str, input: &str) -> String {
    // Git's POSIX tools exercise the shipped hook on Windows, without a cross build.
    let shell = if cfg!(windows) {
        "C:/Program Files/Git/bin/sh.exe"
    } else {
        "sh"
    };
    let scratch = std::env::temp_dir().join(format!("tterm-hook-input-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("ps"), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(scratch.join("ps"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
    }
    let mut paths = vec![scratch.clone()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut child = Command::new(shell)
        .args(["-c", command])
        .env("PATH", std::env::join_paths(paths).unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn claude_hook_transports_valid_session_and_cwd_as_json() {
    let output = run_command(
        &crate::agent_integration::claude_code::hook_command("done"),
        r#"{"session_id":"abc-123","cwd":"/John’s files;100%"}"#,
    );
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        json["terminalSequence"],
        "\u{1b}]777;tterm-agent;claude;done;abc-123;direct;%2F%4A%6F%68%6E%E2%80%99%73%20%66%69%6C%65%73%3B%31%30%30%25\u{7}"
    );
}

#[test]
fn codex_hook_wrapper_preserves_json_parser_and_broadcast_scope() {
    let command = crate::agent_integration::codex::hook_command("done");
    // ps is empty here: exercise both quoting layers without writing any terminal.
    let output = run_command(&command, r#"{"session_id":"abc-123","cwd":"/a;100%"}"#);
    assert!(output.is_empty());
}

#[test]
fn reads_only_top_level_metadata_and_decodes_json_escapes() {
    assert_eq!(
        run(
            r#"{"session_id":"abc-123","cwd":"C:\\John’s files;中","tool_input":{"session_id":"wrong","cwd":"/wrong"}}"#
        ),
        "abc-123;%43%3A%5C%4A%6F%68%6E%E2%80%99%73%20%66%69%6C%65%73%3B%E4%B8%AD"
    );
    assert_eq!(
        run(r#"{"cwd":"/\u4e2d\ud83d\ude00","session_id":"abc\u002d123"}"#),
        "abc-123;%2F%E4%B8%AD%F0%9F%98%80"
    );
}

#[test]
fn rejects_injected_ids_control_directories_and_malformed_json() {
    for cwd in [
        "/bad\nline",
        "/bad\rline",
        "/bad\tline",
        "/bad\0line",
        "/bad\u{1b}line",
        "/bad\u{85}line",
    ] {
        let input = serde_json::json!({"session_id":"abc-123", "cwd":cwd}).to_string();
        let result = run(&input);
        assert!(result == "abc-123;" || result == ";", "{result}");
    }
    assert_eq!(
        run(r#"{"session_id":"abc; rm -rf ~","cwd":"/safe"}"#),
        ";%2F%73%61%66%65"
    );
    for input in [
        "{",
        "{} trailing",
        r#"{"session_id":"a","session_id":"b"}"#,
        r#"{"cwd":"\ud800"}"#,
    ] {
        assert_eq!(run(input), ";");
    }
}
