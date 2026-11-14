//! Specification tests transcribed from `src/lib/commandComposer.test.ts`
//! and `src/lib/composerRun.test.ts`.
//!
//! The bar's target registry, its text state and its localStorage flags stay
//! in the WebView, so those cases remain frontend tests.

use crate::composer::input::{
    caret_on_first_line, caret_on_last_line, composer_input, history_suggestion,
};
use crate::composer::run::{
    ComposerPromptEvent, ComposerRun, RunLineStatus, continue_run, finish_line, is_run_active,
    prompt_reached, retry_line, start_run, stop_run,
};

fn statuses(run: &ComposerRun) -> Vec<RunLineStatus> {
    run.lines.iter().map(|line| line.status).collect()
}

fn lines(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn prompt(
    exit_code: Option<i32>,
    started_command: Option<bool>,
    status: Option<i32>,
) -> ComposerPromptEvent {
    ComposerPromptEvent::Prompt {
        exit_code,
        started_command,
        status,
    }
}

#[test]
fn sends_one_line_followed_by_enter() {
    assert_eq!(composer_input("ls -la", true), "ls -la\r");
    assert_eq!(composer_input("ls -la", false), "ls -la\r");
}

#[test]
fn sends_an_empty_text_as_a_bare_enter() {
    assert_eq!(composer_input("", true), "\r");
}

#[test]
fn drops_trailing_line_breaks_which_would_run_empty_commands() {
    assert_eq!(composer_input("uptime\n\n", true), "uptime\r");
}

#[test]
fn pastes_several_lines_when_the_program_asked_for_bracketed_paste() {
    assert_eq!(
        composer_input("cd /tmp\r\nls", true),
        "\u{1b}[200~cd /tmp\rls\u{1b}[201~\r"
    );
}

#[test]
fn types_several_lines_as_lines_otherwise() {
    assert_eq!(composer_input("cd /tmp\nls", false), "cd /tmp\rls\r");
}

#[test]
fn keeps_an_end_marker_in_the_text_from_ending_the_paste_early() {
    assert_eq!(
        composer_input("echo a\n\u{1b}[201~rm x", true),
        "\u{1b}[200~echo a\rrm x\u{1b}[201~\r"
    );
}

#[test]
fn tells_whether_the_caret_is_on_the_first_or_last_line() {
    let text = "one\ntwo";
    assert!(caret_on_first_line(text, 2));
    assert!(!caret_on_first_line(text, 5));
    assert!(!caret_on_last_line(text, 2));
    assert!(caret_on_last_line(text, 5));
}

#[test]
fn offers_the_newest_command_that_goes_on_from_the_text() {
    let history = lines(&["git status", "git stash pop", "ls -la"]);
    assert_eq!(
        history_suggestion("git st", &history).as_deref(),
        Some("git status")
    );
    assert_eq!(
        history_suggestion("git stash", &history).as_deref(),
        Some("git stash pop")
    );
}

#[test]
fn offers_nothing_for_an_empty_text_a_whole_command_or_several_lines() {
    let history = lines(&["git status", "git stash pop", "ls -la"]);
    assert_eq!(history_suggestion("", &history), None);
    assert_eq!(history_suggestion("ls -la", &history), None);
    assert_eq!(history_suggestion("cd /tmp\ngit", &history), None);
}

#[test]
fn runs_the_lines_in_turn_and_ends_after_the_last() {
    let mut run = start_run(&lines(&["a", "b"]));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Running);
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Running, RunLineStatus::Pending]
    );

    run = finish_line(&run, Some(0));
    assert_eq!(run.current, 1);
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Succeeded, RunLineStatus::Running]
    );

    run = finish_line(&run, None);
    assert_eq!(run.phase, crate::composer::run::RunPhase::Done);
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Succeeded, RunLineStatus::Succeeded]
    );
    assert!(!is_run_active(Some(&run)));
}

#[test]
fn holds_the_rest_when_a_line_fails() {
    let run = finish_line(&start_run(&lines(&["a", "b"])), Some(2));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Failed);
    assert_eq!(run.current, 0);
    assert_eq!(
        run.lines[0],
        crate::composer::run::RunLine {
            text: "a".to_owned(),
            status: RunLineStatus::Failed,
            exit_code: Some(2),
        }
    );
    assert!(is_run_active(Some(&run)));
    // A prompt that comes while the run waits on the user changes nothing.
    assert_eq!(finish_line(&run, Some(0)), run);
}

#[test]
fn runs_a_failed_line_again_on_retry() {
    let run = retry_line(&finish_line(&start_run(&lines(&["a", "b"])), Some(1)));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Running);
    assert_eq!(run.current, 0);
    assert_eq!(
        run.lines[0],
        crate::composer::run::RunLine {
            text: "a".to_owned(),
            status: RunLineStatus::Running,
            exit_code: None,
        }
    );
}

#[test]
fn goes_on_past_a_failed_line_which_stays_failed() {
    let run = continue_run(&finish_line(&start_run(&lines(&["a", "b"])), Some(1)));
    assert_eq!(run.current, 1);
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Failed, RunLineStatus::Running]
    );
}

#[test]
fn ends_when_the_failed_line_was_the_last() {
    let run = continue_run(&finish_line(&start_run(&lines(&["a"])), Some(1)));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Done);
}

#[test]
fn waits_past_a_prompt_at_which_the_shell_ran_nothing() {
    // Enter pressed in the terminal while "a" ran gives a prompt before "b" is read.
    let mut run = prompt_reached(
        &start_run(&lines(&["a", "b", "c"])),
        &prompt(Some(0), Some(true), Some(0)),
    );
    run = prompt_reached(&run, &prompt(None, Some(false), Some(0)));
    assert_eq!(
        statuses(&run),
        vec![
            RunLineStatus::Succeeded,
            RunLineStatus::Running,
            RunLineStatus::Pending
        ]
    );

    run = prompt_reached(&run, &prompt(Some(0), Some(true), Some(0)));
    assert_eq!(
        statuses(&run),
        vec![
            RunLineStatus::Succeeded,
            RunLineStatus::Succeeded,
            RunLineStatus::Running
        ]
    );
}

#[test]
fn ends_a_comment_and_fails_a_line_the_shell_cant_parse_with_nothing_run() {
    let mut run = prompt_reached(
        &start_run(&lines(&["# note", "fi", "c"])),
        &prompt(None, Some(false), None),
    );
    assert_eq!(
        statuses(&run),
        vec![
            RunLineStatus::Succeeded,
            RunLineStatus::Running,
            RunLineStatus::Pending
        ]
    );

    run = prompt_reached(&run, &prompt(None, Some(false), Some(2)));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Failed);
    assert_eq!(run.lines[1].exit_code, Some(2));
}

#[test]
fn takes_every_prompt_as_a_lines_end_in_a_shell_that_marks_no_output() {
    let run = prompt_reached(&start_run(&lines(&["a", "b"])), &prompt(None, None, None));
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Succeeded, RunLineStatus::Running]
    );
    assert_eq!(
        prompt_reached(&run, &ComposerPromptEvent::Ended).phase,
        crate::composer::run::RunPhase::Stopped
    );
}

#[test]
fn stops_with_the_running_line_left_unfinished() {
    let run = stop_run(&start_run(&lines(&["a", "b"])));
    assert_eq!(run.phase, crate::composer::run::RunPhase::Stopped);
    assert_eq!(
        statuses(&run),
        vec![RunLineStatus::Unfinished, RunLineStatus::Pending]
    );
    assert_eq!(stop_run(&run), run);
}
