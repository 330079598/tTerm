use super::{
    BufferLine, CommandMarkViewport, CommandMarks, ShellPromptState, join_buffer_lines,
    parse_command_mark,
};
use crate::command_mark::CommandMark;

mod marker_lifecycle;

const PROMPT: &str = "\x1b]133;A\x1b\\";
const OUTPUT: &str = "\x1b]133;C\x1b\\";

fn done(code: Option<i64>) -> String {
    match code {
        Some(code) => format!("\x1b]133;D;{code}\x1b\\"),
        None => "\x1b]133;D\x1b\\".to_string(),
    }
}

struct FakeViewport {
    normal: bool,
    lines: Vec<String>,
    cursor_row: usize,
    cursor_column: usize,
    viewport: i64,
    at_bottom: bool,
    rows: i64,
}

impl FakeViewport {
    fn new(rows: i64) -> Self {
        Self {
            normal: true,
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_column: 0,
            viewport: 0,
            at_bottom: true,
            rows,
        }
    }

    fn write(&mut self, marks: &mut CommandMarks, data: &str) {
        let mut rest = data;
        while !rest.is_empty() {
            if rest.starts_with("\x1b]133;") || rest.starts_with("\x1b]633;") {
                let prefix = if rest.starts_with("\x1b]133;") {
                    "\x1b]133;"
                } else {
                    "\x1b]633;"
                };
                let after_prefix = &rest[prefix.len()..];
                let (end, terminator_len) = if let Some(index) = after_prefix.find("\x1b\\") {
                    (index, 2)
                } else if let Some(index) = after_prefix.find('\u{0007}') {
                    (index, 1)
                } else {
                    break;
                };
                marks.handle_mark(&after_prefix[..end], self);
                rest = &after_prefix[end + terminator_len..];
                continue;
            }
            if rest.starts_with("\x1b[?1049h") {
                self.normal = false;
                rest = &rest["\x1b[?1049h".len()..];
                continue;
            }
            if rest.starts_with("\x1b[?1049l") {
                self.normal = true;
                rest = &rest["\x1b[?1049l".len()..];
                continue;
            }
            if rest.starts_with("\r\n") {
                self.newline();
                rest = &rest[2..];
                continue;
            }
            if rest.starts_with('\n') {
                self.newline();
                rest = &rest[1..];
                continue;
            }
            if rest.starts_with('\r') {
                self.cursor_column = 0;
                rest = &rest[1..];
                continue;
            }
            let character = rest.chars().next().expect("the rest is not empty");
            self.put(character);
            rest = &rest[character.len_utf8()..];
        }
    }

    fn put(&mut self, character: char) {
        let text = &mut self.lines[self.cursor_row];
        let mut chars: Vec<char> = text.chars().collect();
        while chars.len() < self.cursor_column {
            chars.push(' ');
        }
        if self.cursor_column == chars.len() {
            chars.push(character);
        } else {
            chars[self.cursor_column] = character;
        }
        *text = chars.into_iter().collect();
        self.cursor_column += 1;
    }

    fn newline(&mut self) {
        self.cursor_row += 1;
        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
        }
        self.cursor_column = 0;
    }

    fn base(&self) -> i64 {
        self.lines.len().saturating_sub(self.rows as usize) as i64
    }

    fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_column = 0;
    }

    fn line(&self, line: i64) -> Option<BufferLine> {
        let row = usize::try_from(line).ok()?;
        self.lines.get(row).map(|text| BufferLine {
            is_wrapped: false,
            text: text.trim_end().to_string(),
        })
    }
}

impl CommandMarkViewport for FakeViewport {
    fn is_normal_screen(&self) -> bool {
        self.normal
    }

    fn cursor_line(&self) -> i64 {
        self.cursor_row as i64
    }

    fn cursor_column(&self) -> usize {
        self.cursor_column
    }

    fn viewport_line(&self) -> i64 {
        if self.at_bottom {
            self.base_line()
        } else {
            self.viewport
        }
    }

    fn base_line(&self) -> i64 {
        self.base()
    }

    fn scroll_to_line(&mut self, line: i64) {
        self.viewport = line.clamp(0, self.base());
        self.at_bottom = false;
    }

    fn scroll_to_bottom(&mut self) {
        self.viewport = self.base();
        self.at_bottom = true;
    }
}

fn setup(rows: i64) -> (FakeViewport, CommandMarks) {
    (FakeViewport::new(rows), CommandMarks::new())
}

fn output_text(marks: &CommandMarks, view: &FakeViewport) -> Option<String> {
    marks.last_output_text(|line| view.line(line))
}

#[test]
fn reads_prompt_output_and_done_marks() {
    assert_eq!(parse_command_mark("A"), Some(CommandMark::Prompt));
    assert_eq!(
        parse_command_mark("A;cl=m;aid=1"),
        Some(CommandMark::Prompt)
    );
    assert_eq!(parse_command_mark("C"), Some(CommandMark::Output));
    assert_eq!(
        parse_command_mark("D;0"),
        Some(CommandMark::Done { exit_code: Some(0) })
    );
    assert_eq!(
        parse_command_mark("D;127"),
        Some(CommandMark::Done {
            exit_code: Some(127)
        })
    );
    assert_eq!(
        parse_command_mark("D"),
        Some(CommandMark::Done { exit_code: None })
    );
    assert_eq!(
        parse_command_mark("D;x"),
        Some(CommandMark::Done { exit_code: None })
    );
}

#[test]
fn ignores_other_marks() {
    assert_eq!(parse_command_mark("B"), None);
    assert_eq!(parse_command_mark("E;ls -la"), None);
    assert_eq!(parse_command_mark(""), None);
}

#[test]
fn runs_wrapped_rows_on_and_drops_trailing_blank_lines() {
    let line = |text: &str, is_wrapped: bool| BufferLine {
        is_wrapped,
        text: text.to_string(),
    };
    assert_eq!(
        join_buffer_lines(&[
            line("abc", false),
            line("def", true),
            line("x", false),
            line("", false),
            line("", false),
        ]),
        "abcdef\nx"
    );
}

#[test]
fn copies_the_output_between_the_c_and_d_marks() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}$ ls\r\n{OUTPUT}a.txt\r\nb.txt\r\n{}{PROMPT}$ ",
            done(Some(0))
        ),
    );
    assert_eq!(output_text(&marks, &view).as_deref(), Some("a.txt\nb.txt"));
}

#[test]
fn keeps_output_that_ends_without_a_line_break() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}$ printf hi\r\n{OUTPUT}hi{}\r\n{PROMPT}$ ",
            done(Some(0))
        ),
    );
    assert_eq!(output_text(&marks, &view).as_deref(), Some("hi"));
}

#[test]
fn starts_output_on_enter_when_the_shell_sends_no_c_mark() {
    let (mut view, mut marks) = setup(24);
    view.write(&mut marks, &format!("{PROMPT}PS> dir"));
    assert!(marks.handle_enter(&mut view));
    view.write(
        &mut marks,
        &format!("\r\nout 1\r\nout 2\r\n{}{PROMPT}PS> ", done(Some(1))),
    );
    assert_eq!(output_text(&marks, &view).as_deref(), Some("out 1\nout 2"));
}

#[test]
fn ignores_enter_on_an_empty_line() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}$ ls\r\n{OUTPUT}a.txt\r\n{}{PROMPT}$ ",
            done(Some(0))
        ),
    );
    marks.handle_enter(&mut view);
    view.write(&mut marks, &format!("\r\n{}{PROMPT}$ ", done(Some(0))));
    assert_eq!(output_text(&marks, &view).as_deref(), Some("a.txt"));
}

#[test]
fn knows_no_output_for_prompts_without_a_d_status() {
    let (mut view, mut marks) = setup(24);
    view.write(&mut marks, &format!("{PROMPT}C:\\> dir"));
    marks.handle_enter(&mut view);
    view.write(&mut marks, &format!("\r\nfiles\r\n\r\n{PROMPT}C:\\> "));
    assert_eq!(marks.last_output_range(), None);
}

#[test]
fn reports_an_empty_output_for_a_command_that_printed_nothing() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!("{PROMPT}$ true\r\n{OUTPUT}{}{PROMPT}$ ", done(Some(0))),
    );
    assert_eq!(output_text(&marks, &view).as_deref(), Some(""));
    assert!(!marks.select_last_output());
}

#[test]
fn knows_when_the_shell_waits_at_a_prompt() {
    let (mut view, mut marks) = setup(24);
    view.write(&mut marks, &format!("{PROMPT}$ cat big.bin\r\n{OUTPUT}"));
    assert!(!marks.is_at_prompt(&view));
    view.write(&mut marks, &format!("junk\r\n{}{PROMPT}$ ", done(Some(0))));
    assert!(marks.is_at_prompt(&view));
    view.write(&mut marks, "vim");
    assert!(marks.is_at_prompt(&view));
    view.write(&mut marks, &format!("\r\n{OUTPUT}\x1b[?1049h"));
    assert!(!marks.is_at_prompt(&view));
}

#[test]
fn knows_the_prompt_from_enter_when_the_shell_sends_no_c() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}PS> dir\r\nout\r\n{}{PROMPT}PS> python",
            done(Some(0))
        ),
    );
    assert!(marks.is_at_prompt(&view));
    marks.handle_enter(&mut view);
    view.write(&mut marks, "\r\n>>> ");
    assert!(!marks.is_at_prompt(&view));
}

#[test]
fn does_not_claim_a_prompt_for_shells_without_d_marks() {
    let (mut view, mut marks) = setup(24);
    view.write(&mut marks, &format!("{PROMPT}C:\\> dir"));
    marks.handle_enter(&mut view);
    view.write(&mut marks, &format!("\r\nfiles\r\n\r\n{PROMPT}C:\\> "));
    assert!(!marks.is_at_prompt(&view));
}

#[test]
fn knows_when_a_command_runs_from_its_c_and_d_marks() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}$ ls\r\n{OUTPUT}a\r\n{}{PROMPT}$ claude",
            done(Some(0))
        ),
    );
    assert!(!marks.is_running_program(&view));
    assert!(!marks.handle_enter(&mut view));
    view.write(&mut marks, &format!("\r\n{OUTPUT}"));
    assert!(marks.is_running_program(&view));
    view.write(&mut marks, &format!("> hi\r\n{}", done(Some(0))));
    assert!(!marks.is_running_program(&view));
}

#[test]
fn knows_a_line_went_to_a_program_when_no_directory_report_followed_enter() {
    let (mut view, mut marks) = setup(24);
    marks.set_enabled(false);
    marks.handle_enter(&mut view);
    assert!(!marks.awaits_prompt());

    marks.note_cwd_report();
    assert!(!marks.awaits_prompt());
    marks.handle_enter(&mut view);
    assert!(marks.awaits_prompt());
    marks.handle_enter(&mut view);
    assert!(marks.awaits_prompt());
    marks.note_cwd_report();
    assert!(!marks.awaits_prompt());
}

#[test]
fn knows_when_a_command_runs_from_enter_without_c_marks() {
    let (mut view, mut marks) = setup(24);
    view.write(&mut marks, &format!("{PROMPT}C:\\> claude"));
    assert!(!marks.is_running_program(&view));
    assert!(marks.handle_enter(&mut view));
    assert!(marks.is_running_program(&view));
    view.write(&mut marks, "\r\n> hi");
    assert!(!marks.handle_enter(&mut view));
    assert!(marks.is_running_program(&view));
    view.write(&mut marks, &format!("\r\n{PROMPT}C:\\> "));
    assert!(!marks.is_running_program(&view));
}

#[test]
fn counts_a_full_screen_program_as_running_marks_or_not() {
    let (mut view, mut marks) = setup(24);
    assert!(!marks.is_running_program(&view));
    view.write(&mut marks, "\x1b[?1049h");
    assert!(marks.is_running_program(&view));
    view.write(&mut marks, &format!("\x1b[?1049l{PROMPT}$ ls\r\n{OUTPUT}"));
    assert!(marks.is_running_program(&view));
    marks.set_enabled(false);
    assert!(!marks.is_running_program(&view));
}

#[test]
fn does_not_claim_a_prompt_before_any_mark_or_when_turned_off() {
    let (mut view, mut marks) = setup(24);
    assert!(!marks.is_at_prompt(&view));
    view.write(
        &mut marks,
        &format!("{PROMPT}$ ls\r\n{OUTPUT}a\r\n{}{PROMPT}$ ", done(Some(0))),
    );
    marks.set_enabled(false);
    assert!(!marks.is_at_prompt(&view));
}

#[test]
fn forgets_everything_when_turned_off() {
    let (mut view, mut marks) = setup(24);
    marks.set_enabled(false);
    view.write(
        &mut marks,
        &format!("{PROMPT}$ ls\r\n{OUTPUT}a\r\n{}{PROMPT}$ ", done(Some(0))),
    );
    assert_eq!(marks.last_output_range(), None);
    assert!(!marks.scroll_to_previous_prompt(&mut view));
}

#[test]
fn jumps_between_prompts_and_back_to_the_bottom() {
    let (mut view, mut marks) = setup(5);
    for command in 0..3 {
        view.write(&mut marks, &format!("{PROMPT}$ cmd{command}\r\n{OUTPUT}"));
        let out: String = (0..5)
            .map(|line| format!("out {command}.{line}\r\n"))
            .collect();
        view.write(&mut marks, &out);
        view.write(&mut marks, &done(Some(0)));
    }
    view.write(&mut marks, &format!("{PROMPT}$ "));
    assert_eq!(view.viewport_line(), view.base());

    assert!(marks.scroll_to_previous_prompt(&mut view));
    assert_eq!(view.viewport, 12);
    marks.scroll_to_previous_prompt(&mut view);
    assert_eq!(view.viewport, 6);
    marks.scroll_to_previous_prompt(&mut view);
    assert_eq!(view.viewport, 0);
    marks.scroll_to_previous_prompt(&mut view);
    assert_eq!(view.viewport, 0);

    marks.scroll_to_next_prompt(&mut view);
    assert_eq!(view.viewport, 6);
    marks.scroll_to_next_prompt(&mut view);
    assert_eq!(view.viewport, 12);
    marks.scroll_to_next_prompt(&mut view);
    assert_eq!(view.viewport_line(), view.base());
}

#[test]
fn keeps_going_up_past_prompts_that_cannot_scroll_to_the_top() {
    let (mut view, mut marks) = setup(10);
    for command in 0..3 {
        view.write(
            &mut marks,
            &format!(
                "{PROMPT}$ cmd{command}\r\n{OUTPUT}out\r\n\r\n{}",
                done(Some(0))
            ),
        );
    }
    view.write(&mut marks, &format!("{PROMPT}$ \r\n\r\n\r\n"));
    assert!(view.base() > 0);

    marks.scroll_to_previous_prompt(&mut view);
    marks.scroll_to_previous_prompt(&mut view);
    marks.scroll_to_previous_prompt(&mut view);
    assert_eq!(view.viewport, 0);
}

#[test]
fn tells_each_prompt_with_the_status_of_the_command_before_it() {
    let (mut view, mut marks) = setup(24);
    marks.listen_prompt_events();
    view.write(
        &mut marks,
        &format!("{PROMPT}$ false\r\n{OUTPUT}{}{PROMPT}$ ", done(Some(1))),
    );
    marks.handle_enter(&mut view);
    view.write(&mut marks, &format!("\r\n{}{PROMPT}$ ", done(Some(1))));
    assert_eq!(
        marks.take_prompt_events(),
        vec![
            super::PromptEvent::Prompt {
                exit_code: None,
                started_command: None,
                status: None,
            },
            super::PromptEvent::Prompt {
                exit_code: Some(1),
                started_command: Some(true),
                status: Some(1),
            },
            super::PromptEvent::Prompt {
                exit_code: None,
                started_command: Some(false),
                status: Some(1),
            },
        ]
    );
}

#[test]
fn knows_a_marked_prompt_in_shells_that_mark_no_finished_command_yet() {
    let (mut view, mut marks) = setup(24);
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Unmarked);
    view.write(&mut marks, &format!("{PROMPT}> "));
    assert!(!marks.is_at_prompt(&view));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Prompt);
    view.write(&mut marks, &format!("sleep 9\r\n{OUTPUT}"));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Busy);
}

#[test]
fn keeps_knowing_the_prompt_after_the_terminal_is_cleared() {
    let (mut view, mut marks) = setup(24);
    view.write(
        &mut marks,
        &format!(
            "{PROMPT}$ ls\r\n{OUTPUT}a.txt\r\n{}{PROMPT}$ ",
            done(Some(0))
        ),
    );
    view.clear();
    marks.dispose_markers();
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Prompt);
}

#[test]
fn tells_listeners_the_marks_ended_when_turned_off_or_disposed() {
    let (_view, mut marks) = setup(24);
    marks.listen_prompt_events();
    marks.set_enabled(false);
    marks.dispose();
    marks.stop_listening_prompt_events();
    assert_eq!(marks.take_prompt_events(), vec![super::PromptEvent::Ended]);
}

#[test]
fn oracle_prompt_events_end_with_the_listener_and_do_not_resume() {
    let (mut view, mut marks) = setup(24);
    marks.listen_prompt_events();
    view.write(&mut marks, PROMPT);
    assert_eq!(
        marks.take_prompt_events(),
        vec![super::PromptEvent::Prompt {
            exit_code: None,
            started_command: None,
            status: None,
        }]
    );

    marks.set_enabled(false);
    assert_eq!(marks.take_prompt_events(), vec![super::PromptEvent::Ended]);
    marks.set_enabled(true);
    view.write(&mut marks, PROMPT);
    assert!(marks.take_prompt_events().is_empty());
    marks.stop_listening_prompt_events();
    view.write(&mut marks, PROMPT);
    assert!(marks.take_prompt_events().is_empty());
}

#[test]
fn oracle_parse_command_mark_accepts_extra_fields_and_negative_zero() {
    assert_eq!(
        parse_command_mark("D;1;2"),
        Some(CommandMark::Done { exit_code: Some(1) })
    );
    assert_eq!(
        parse_command_mark("D;-0"),
        Some(CommandMark::Done { exit_code: Some(0) })
    );
    assert_eq!(
        parse_command_mark("D;-7"),
        Some(CommandMark::Done {
            exit_code: Some(-7)
        })
    );
    assert_eq!(parse_command_mark("C;ls -la"), Some(CommandMark::Output));
}

#[test]
fn oracle_join_buffer_lines_uses_javascript_whitespace_and_wraps() {
    assert_eq!(join_buffer_lines(&[]), "");
    assert_eq!(
        join_buffer_lines(&[BufferLine {
            is_wrapped: false,
            text: String::new(),
        }]),
        ""
    );
    assert_eq!(
        join_buffer_lines(&[
            BufferLine {
                is_wrapped: false,
                text: "a".to_string(),
            },
            BufferLine {
                is_wrapped: false,
                text: "\u{3000}\t".to_string(),
            },
        ]),
        "a"
    );
    assert_eq!(
        join_buffer_lines(&[BufferLine {
            is_wrapped: false,
            text: "a\u{00A0}\u{FEFF}".to_string(),
        }]),
        "a"
    );
    assert_eq!(
        join_buffer_lines(&[
            BufferLine {
                is_wrapped: false,
                text: "中文".to_string(),
            },
            BufferLine {
                is_wrapped: true,
                text: "😀".to_string(),
            },
        ]),
        "中文😀"
    );
    assert_eq!(
        join_buffer_lines(&[
            BufferLine {
                is_wrapped: false,
                text: "e\u{0301}".to_string(),
            },
            BufferLine {
                is_wrapped: true,
                text: "😀".to_string(),
            },
        ]),
        "e\u{0301}😀"
    );
    assert_eq!(
        join_buffer_lines(&[BufferLine {
            is_wrapped: false,
            text: "x\n".to_string(),
        }]),
        "x"
    );
    assert_eq!(
        join_buffer_lines(&[BufferLine {
            is_wrapped: false,
            text: "a\u{0085}".to_string(),
        }]),
        "a\u{0085}"
    );
}

#[test]
fn exit_codes_outside_the_shared_i64_field_are_dropped() {
    assert_eq!(
        parse_command_mark("D;9223372036854775808"),
        Some(CommandMark::Done { exit_code: None })
    );
    assert_eq!(
        parse_command_mark("D;9223372036854775807"),
        Some(CommandMark::Done {
            exit_code: Some(i64::MAX)
        })
    );
}
