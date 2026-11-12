use super::super::{BufferLine, CommandMarkViewport, CommandMarks, PromptEvent, ShellPromptState};

struct Viewport {
    cursor: i64,
    column: usize,
    base: i64,
    viewport: i64,
}

impl Viewport {
    fn new() -> Self {
        Self {
            cursor: 0,
            column: 0,
            base: 12,
            viewport: 12,
        }
    }

    fn mark(&mut self, marks: &mut CommandMarks, data: &str, row: i64) {
        self.cursor = row;
        marks.handle_mark(data, self);
    }

    fn clear(&mut self, marks: &mut CommandMarks) {
        marks.dispose_markers();
        self.cursor = 0;
        self.column = 0;
        self.base = 0;
        self.viewport = 0;
    }

    fn trim(&mut self, marks: &mut CommandMarks, count: usize) {
        marks.trim_scrollback(count);
        self.base = (self.base - count as i64).max(0);
        self.viewport = (self.viewport - count as i64).max(0);
    }
}

impl CommandMarkViewport for Viewport {
    fn is_normal_screen(&self) -> bool {
        true
    }
    fn cursor_line(&self) -> i64 {
        self.cursor
    }
    fn cursor_column(&self) -> usize {
        self.column
    }
    fn viewport_line(&self) -> i64 {
        self.viewport
    }
    fn base_line(&self) -> i64 {
        self.base
    }
    fn scroll_to_line(&mut self, line: i64) {
        self.viewport = line.min(self.base);
    }
    fn scroll_to_bottom(&mut self) {
        self.viewport = self.base;
    }
}

fn assert_empty_output(marks: &CommandMarks) {
    assert_eq!(marks.last_output_range(), None);
    assert_eq!(
        marks.last_output_text(|_| Some(BufferLine {
            is_wrapped: false,
            text: "unexpected".to_string(),
        })),
        None
    );
    assert!(!marks.select_last_output());
}

#[test]
fn oracle_clear_and_trim_dispose_prompts_and_move_navigation_and_output() {
    let mut marks = CommandMarks::new();
    let mut view = Viewport::new();
    view.mark(&mut marks, "D;0", 0);
    view.column = 2;
    view.mark(&mut marks, "A", 0);
    view.clear(&mut marks);
    assert!(!marks.handle_enter(&mut view));
    assert!(!marks.is_at_prompt(&view));
    assert!(!marks.is_running_program(&view));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Prompt);
    assert_empty_output(&marks);
    assert!(!marks.scroll_to_previous_prompt(&mut view));
    assert!(!marks.scroll_to_next_prompt(&mut view));

    for (data, row) in [
        ("A", 2),
        ("C", 3),
        ("D;0", 5),
        ("A", 6),
        ("C", 7),
        ("D;7", 9),
        ("A", 10),
    ] {
        view.mark(&mut marks, data, row);
    }
    view.base = 12;
    view.viewport = 12;
    view.trim(&mut marks, 4);
    assert!(marks.is_at_prompt(&view));
    assert!(!marks.is_running_program(&view));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Prompt);
    assert_eq!(marks.last_output_range(), Some(3..=4));
    assert_eq!(
        marks.last_output_text(|row| Some(BufferLine {
            is_wrapped: false,
            text: format!("row {row}"),
        })),
        Some("row 3\nrow 4".to_string())
    );
    assert!(marks.select_last_output());
    assert!(marks.scroll_to_previous_prompt(&mut view));
    assert_eq!(view.viewport, 2);

    view.trim(&mut marks, 1);
    assert!(marks.scroll_to_next_prompt(&mut view));
    assert_eq!(view.viewport, 5);
    assert!(marks.scroll_to_previous_prompt(&mut view));
    assert_eq!(view.viewport, 1);

    view.trim(&mut marks, 6);
    assert!(!marks.handle_enter(&mut view));
    assert!(!marks.is_at_prompt(&view));
    assert!(!marks.is_running_program(&view));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Prompt);
    assert_empty_output(&marks);
    assert!(!marks.scroll_to_previous_prompt(&mut view));
    assert!(!marks.scroll_to_next_prompt(&mut view));
}

#[test]
fn oracle_disposal_preserves_running_state_and_status_but_not_exit_code() {
    let mut marks = CommandMarks::new();
    let mut view = Viewport::new();
    marks.listen_prompt_events();
    view.mark(&mut marks, "A", 1);
    view.mark(&mut marks, "C", 2);
    view.clear(&mut marks);
    assert!(!marks.is_at_prompt(&view));
    assert!(marks.is_running_program(&view));
    assert_eq!(marks.prompt_state(&view), ShellPromptState::Busy);
    assert_empty_output(&marks);
    view.mark(&mut marks, "D;7", 0);
    view.mark(&mut marks, "A", 0);
    assert_eq!(
        marks.take_prompt_events(),
        vec![
            PromptEvent::Prompt {
                exit_code: None,
                started_command: None,
                status: None
            },
            PromptEvent::Prompt {
                exit_code: None,
                started_command: Some(true),
                status: Some(7)
            },
        ]
    );
    view.mark(&mut marks, "C", 1);
    view.mark(&mut marks, "D;3", 2);
    view.trim(&mut marks, 1);
    view.mark(&mut marks, "A", 3);
    assert_eq!(
        marks.take_prompt_events(),
        vec![PromptEvent::Prompt {
            exit_code: None,
            started_command: Some(true),
            status: Some(3),
        }]
    );
}

#[test]
fn oracle_disposed_output_or_end_marker_invalidates_output_even_with_a_live_prompt() {
    for (output, end, trim) in [(2, 8, 3), (11, 1, 2)] {
        let mut marks = CommandMarks::new();
        let mut view = Viewport::new();
        view.mark(&mut marks, "A", 10);
        view.mark(&mut marks, "C", output);
        view.mark(&mut marks, "D;0", end);
        view.trim(&mut marks, trim);
        assert_empty_output(&marks);
    }
}
