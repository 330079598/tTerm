use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{
    LOGIN_SCRIPT_FALLBACK_IDLE_MS, LOGIN_SCRIPT_PROMPT_IDLE_MS, LoginScriptRunner,
    LoginScriptRunnerOptions, looks_like_shell_prompt, parse_login_script,
};

struct Shared {
    cursor_line: Rc<RefCell<String>>,
    at_password_prompt: Rc<Cell<bool>>,
    sent: Rc<RefCell<Vec<String>>>,
}

fn runner(lines: &[&str]) -> (LoginScriptRunner, Shared) {
    let cursor_line = Rc::new(RefCell::new(String::new()));
    let at_password_prompt = Rc::new(Cell::new(false));
    let sent = Rc::new(RefCell::new(Vec::new()));
    let options = LoginScriptRunnerOptions {
        lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        send: {
            let sent = sent.clone();
            Box::new(move |line| sent.borrow_mut().push(line.to_owned()))
        },
        read_cursor_line: {
            let cursor_line = cursor_line.clone();
            Box::new(move || cursor_line.borrow().clone())
        },
        is_at_password_prompt: {
            let at_password_prompt = at_password_prompt.clone();
            Box::new(move || at_password_prompt.get())
        },
        prompt_idle_ms: None,
        fallback_idle_ms: None,
    };
    (
        LoginScriptRunner::new(options, 0),
        Shared {
            cursor_line,
            at_password_prompt,
            sent,
        },
    )
}

#[test]
fn splits_on_any_line_ending_and_skips_blank_lines() {
    assert_eq!(
        parse_login_script(Some("cd /srv/app\r\n\n  \rsudo -i  \n")),
        vec!["cd /srv/app".to_owned(), "sudo -i".to_owned()]
    );
}

#[test]
fn keeps_leading_indentation() {
    assert_eq!(
        parse_login_script(Some("  echo hi")),
        vec!["  echo hi".to_owned()]
    );
}

#[test]
fn treats_a_missing_script_as_empty() {
    assert!(parse_login_script(None).is_empty());
    assert!(parse_login_script(Some("")).is_empty());
    assert!(parse_login_script(Some(" \r\n\t")).is_empty());
}

#[test]
fn recognizes_common_prompts() {
    assert!(looks_like_shell_prompt("user@host:~$ "));
    assert!(looks_like_shell_prompt("root@host:~# "));
    assert!(looks_like_shell_prompt("host% "));
    assert!(looks_like_shell_prompt("PS C:\\> "));
    assert!(looks_like_shell_prompt("~/src ❯ "));
    assert!(looks_like_shell_prompt("~/src ➜ "));
    assert!(looks_like_shell_prompt("~/src λ "));
    assert!(looks_like_shell_prompt("~/src » "));
    assert!(looks_like_shell_prompt("foo$\n"));
}

#[test]
fn rejects_password_prompts_and_plain_output() {
    assert!(!looks_like_shell_prompt("[sudo] password for user: "));
    assert!(!looks_like_shell_prompt(
        "Last login: Mon Oct  5 10:00:00 2026"
    ));
    assert!(!looks_like_shell_prompt(""));
    assert!(!looks_like_shell_prompt("foo$\nbar"));
}

#[test]
fn types_one_command_per_quiet_prompt() {
    let (mut runner, shared) = runner(&["cd /srv", "ls"]);
    *shared.cursor_line.borrow_mut() = "user@host:~$ ".to_owned();
    runner.note_output(0);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS);
    assert_eq!(*shared.sent.borrow(), vec!["cd /srv".to_owned()]);

    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS + LOGIN_SCRIPT_FALLBACK_IDLE_MS * 2);
    assert_eq!(*shared.sent.borrow(), vec!["cd /srv".to_owned()]);

    *shared.cursor_line.borrow_mut() = "user@host:/srv$ ".to_owned();
    runner.note_output(LOGIN_SCRIPT_PROMPT_IDLE_MS);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS * 2);
    assert_eq!(
        *shared.sent.borrow(),
        vec!["cd /srv".to_owned(), "ls".to_owned()]
    );
    assert!(runner.done());
}

#[test]
fn waits_until_output_stops() {
    let (mut runner, shared) = runner(&["uptime"]);
    *shared.cursor_line.borrow_mut() = "user@host:~$ ".to_owned();
    runner.note_output(0);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS - 1);
    runner.note_output(LOGIN_SCRIPT_PROMPT_IDLE_MS - 1);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS - 1 + LOGIN_SCRIPT_PROMPT_IDLE_MS - 1);
    assert!(shared.sent.borrow().is_empty());
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS * 2);
    assert_eq!(*shared.sent.borrow(), vec!["uptime".to_owned()]);
}

#[test]
fn falls_back_to_a_longer_wait_when_the_prompt_is_unusual() {
    let (mut runner, shared) = runner(&["id"]);
    *shared.cursor_line.borrow_mut() = "[custom prompt] ".to_owned();
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS);
    assert!(shared.sent.borrow().is_empty());
    runner.fire_due(LOGIN_SCRIPT_FALLBACK_IDLE_MS);
    assert_eq!(*shared.sent.borrow(), vec!["id".to_owned()]);
}

#[test]
fn leaves_a_password_prompt_to_the_user() {
    let (mut runner, shared) = runner(&["sudo -i", "cd /root"]);
    *shared.cursor_line.borrow_mut() = "user@host:~$ ".to_owned();
    runner.note_output(0);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS);
    assert_eq!(*shared.sent.borrow(), vec!["sudo -i".to_owned()]);

    *shared.cursor_line.borrow_mut() = "[sudo] password for user: ".to_owned();
    shared.at_password_prompt.set(true);
    runner.note_output(LOGIN_SCRIPT_PROMPT_IDLE_MS);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS + LOGIN_SCRIPT_FALLBACK_IDLE_MS * 3);
    assert_eq!(*shared.sent.borrow(), vec!["sudo -i".to_owned()]);

    *shared.cursor_line.borrow_mut() = "root@host:~# ".to_owned();
    shared.at_password_prompt.set(false);
    runner.note_output(LOGIN_SCRIPT_PROMPT_IDLE_MS + LOGIN_SCRIPT_FALLBACK_IDLE_MS * 3);
    runner.fire_due(LOGIN_SCRIPT_PROMPT_IDLE_MS * 2 + LOGIN_SCRIPT_FALLBACK_IDLE_MS * 3);
    assert_eq!(
        *shared.sent.borrow(),
        vec!["sudo -i".to_owned(), "cd /root".to_owned()]
    );
}

#[test]
fn sends_nothing_once_stopped() {
    let (mut runner, shared) = runner(&["ls"]);
    *shared.cursor_line.borrow_mut() = "user@host:~$ ".to_owned();
    runner.stop();
    runner.note_output(0);
    runner.fire_due(LOGIN_SCRIPT_FALLBACK_IDLE_MS);
    assert!(shared.sent.borrow().is_empty());
}

#[test]
fn is_done_at_once_with_no_commands() {
    let (runner, _) = runner(&[]);
    assert!(runner.done());
}
