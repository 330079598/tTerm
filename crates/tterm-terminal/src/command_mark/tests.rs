use super::CommandMark;

#[test]
fn distinguishes_shell_integration_marks_and_exit_codes() {
    assert_ne!(CommandMark::Prompt, CommandMark::Output);
    assert_eq!(
        CommandMark::Done { exit_code: Some(0) },
        CommandMark::Done { exit_code: Some(0) }
    );
    assert_ne!(
        CommandMark::Done { exit_code: Some(0) },
        CommandMark::Done { exit_code: None }
    );
}
