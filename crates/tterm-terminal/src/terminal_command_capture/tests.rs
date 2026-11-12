use super::{
    TerminalCommandCaptureState, capture_terminal_input, empty_command_capture_state,
    parse_shell_integration_command,
};

#[test]
fn starts_from_and_returns_the_frontends_state_shape() {
    assert_eq!(
        empty_command_capture_state(),
        TerminalCommandCaptureState {
            text: String::new(),
            cursor: 0,
            bracketed_paste: false,
            valid: true,
        }
    );
    let typed = capture_terminal_input(&empty_command_capture_state(), "ab");
    assert_eq!(
        typed.state,
        TerminalCommandCaptureState {
            text: "ab".to_owned(),
            cursor: 2,
            bracketed_paste: false,
            valid: true,
        }
    );
    let entered = capture_terminal_input(&typed.state, "\r");
    assert_eq!(
        entered.state,
        TerminalCommandCaptureState {
            text: String::new(),
            cursor: 0,
            bracketed_paste: false,
            valid: true,
        }
    );
}

#[test]
fn captures_typed_commands_on_enter() {
    let typed = capture_terminal_input(&empty_command_capture_state(), "git status");
    assert_eq!(
        capture_terminal_input(&typed.state, "\r").commands,
        vec!["git status".to_owned()]
    );
}

#[test]
fn counts_and_inserts_chinese_and_emoji_as_one_character_each() {
    let typed = capture_terminal_input(&empty_command_capture_state(), "密码🔑");
    assert_eq!(typed.state.cursor, 4);
    assert_eq!(typed.state.text, "密码🔑");
    assert_eq!(
        capture_terminal_input(&typed.state, "\r").commands,
        vec!["密码🔑".to_owned()]
    );
}

#[test]
fn tracks_cursor_edits_and_backspace() {
    let mut result = capture_terminal_input(&empty_command_capture_state(), "git sttus");
    result = capture_terminal_input(&result.state, "\x1b[D\x1b[D\x1b[D");
    result = capture_terminal_input(&result.state, "a");
    assert_eq!(
        capture_terminal_input(&result.state, "\r").commands,
        vec!["git status".to_owned()]
    );
}

#[test]
fn keeps_bracketed_multiline_paste_as_one_command() {
    let pasted = capture_terminal_input(
        &empty_command_capture_state(),
        "\x1b[200~printf 'a'\nprintf 'b'\x1b[201~",
    );
    assert_eq!(
        capture_terminal_input(&pasted.state, "\r").commands,
        vec!["printf 'a'\nprintf 'b'".to_owned()]
    );
}

#[test]
fn captures_a_large_bracketed_paste_in_one_pass() {
    let lines: Vec<String> = (0..20_000)
        .map(|index| format!("echo line {index}"))
        .collect();
    let pasted = capture_terminal_input(
        &empty_command_capture_state(),
        &format!("\x1b[200~{}\x1b[201~", lines.join("\r")),
    );
    assert_eq!(
        capture_terminal_input(&pasted.state, "\r").commands,
        vec![lines.join("\n")]
    );
}

#[test]
fn inserts_pasted_text_at_the_cursor() {
    let mut result = capture_terminal_input(&empty_command_capture_state(), "git  -s");
    result = capture_terminal_input(&result.state, "\x1b[D\x1b[D\x1b[D");
    result = capture_terminal_input(&result.state, "\x1b[200~status\x1b[201~");
    assert_eq!(
        capture_terminal_input(&result.state, "\r").commands,
        vec!["git status -s".to_owned()]
    );
}

#[test]
fn does_not_invent_commands_after_shell_history_navigation() {
    let navigated = capture_terminal_input(&empty_command_capture_state(), "\x1b[A\r");
    assert!(navigated.commands.is_empty());
}

#[test]
fn a_sequence_split_between_chunks_is_not_reassembled() {
    let mut result = capture_terminal_input(&empty_command_capture_state(), "\x1b");
    result = capture_terminal_input(&result.state, "[A\r");
    assert!(result.commands.is_empty());

    let mut result = capture_terminal_input(&empty_command_capture_state(), "\x1b[200");
    result = capture_terminal_input(&result.state, "~abc\x1b[201~");
    result = capture_terminal_input(&result.state, "\r");
    assert!(result.commands.is_empty());
}

#[test]
fn c1_control_characters_are_typed_text() {
    let typed = capture_terminal_input(&empty_command_capture_state(), "\u{0085}");
    assert_eq!(typed.state.text, "\u{0085}");
    assert_eq!(
        capture_terminal_input(&typed.state, "\r").commands,
        vec!["\u{0085}".to_owned()]
    );
}

#[test]
fn ctrl_w_erases_a_unicode_word_and_delete_removes_one_unit() {
    let mut result = capture_terminal_input(&empty_command_capture_state(), "git add 中文文件");
    result = capture_terminal_input(&result.state, "\x17");
    assert_eq!(
        capture_terminal_input(&result.state, "\r").commands,
        vec!["git add".to_owned()]
    );

    let mut result = capture_terminal_input(&empty_command_capture_state(), "a😀 b");
    result = capture_terminal_input(&result.state, "\x17");
    assert_eq!(
        capture_terminal_input(&result.state, "\r").commands,
        vec!["a😀".to_owned()]
    );

    let mut result = capture_terminal_input(&empty_command_capture_state(), "git  -s");
    result = capture_terminal_input(&result.state, "\x1b[D\x1b[D\x1b[D");
    result = capture_terminal_input(&result.state, "\x1b[3~");
    assert_eq!(
        capture_terminal_input(&result.state, "\r").commands,
        vec!["git -s".to_owned()]
    );
}

#[test]
fn reads_command_lines_supplied_by_shell_integration() {
    assert_eq!(
        parse_shell_integration_command("E;docker ps").as_deref(),
        Some("docker ps")
    );
    assert_eq!(
        parse_shell_integration_command("E;printf a; printf b").as_deref(),
        Some("printf a; printf b")
    );
    assert_eq!(parse_shell_integration_command("D;0"), None);
}

#[test]
fn unescapes_the_backslashes_and_control_characters_of_an_e_command_line() {
    assert_eq!(
        parse_shell_integration_command("E;dir C:\\x5ctmp\\x0a\\x09ls \\\\x").as_deref(),
        Some("dir C:\\tmp\n\tls \\x")
    );
    assert_eq!(parse_shell_integration_command("E;\\x20 "), None);
    assert_eq!(
        parse_shell_integration_command("C;dir C:\\x5ctmp").as_deref(),
        Some("dir C:\\x5ctmp")
    );
    assert_eq!(
        parse_shell_integration_command("C;  ls  ").as_deref(),
        Some("ls")
    );
    assert_eq!(
        parse_shell_integration_command("E;\\xZZ").as_deref(),
        Some("\\xZZ")
    );
}
