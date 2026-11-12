use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use tterm_terminal::command_marks::CommandMarkViewport;

use super::{CommandProfile, CommandTracking, CommandTrackingCallbacks, ExecutedCommand};
use crate::agent_status::{
    AgentTracker, AgentTrackerCallbacks, AgentTrackerRegistry, parse_agent_report,
};
use crate::terminal_notifications::TerminalAttentionEvent;

struct Viewport {
    normal: bool,
    cursor_line: i64,
    cursor_column: usize,
    viewport_line: i64,
    base_line: i64,
}

impl CommandMarkViewport for Viewport {
    fn is_normal_screen(&self) -> bool {
        self.normal
    }

    fn cursor_line(&self) -> i64 {
        self.cursor_line
    }

    fn cursor_column(&self) -> usize {
        self.cursor_column
    }

    fn viewport_line(&self) -> i64 {
        self.viewport_line
    }

    fn base_line(&self) -> i64 {
        self.base_line
    }

    fn scroll_to_line(&mut self, line: i64) {
        self.viewport_line = line;
    }

    fn scroll_to_bottom(&mut self) {
        self.viewport_line = self.base_line;
    }
}

struct Harness {
    tracking: CommandTracking,
    view: Viewport,
    executed: Rc<RefCell<Vec<ExecutedCommand>>>,
    attention: Rc<RefCell<Vec<TerminalAttentionEvent>>>,
    running: Rc<RefCell<Vec<Option<String>>>>,
    agent_events: Rc<RefCell<Vec<String>>>,
    agent_sessions: Rc<RefCell<Vec<Option<String>>>>,
    recorded: Rc<Cell<usize>>,
    fill_pending: Rc<Cell<bool>>,
}

impl Harness {
    fn new() -> Self {
        let executed = Rc::new(RefCell::new(Vec::new()));
        let attention = Rc::new(RefCell::new(Vec::new()));
        let running = Rc::new(RefCell::new(Vec::new()));
        let agent_events = Rc::new(RefCell::new(Vec::new()));
        let agent_sessions = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::new(Cell::new(0));
        let fill_pending = Rc::new(Cell::new(false));

        let executed_sink = Rc::clone(&executed);
        let attention_sink = Rc::clone(&attention);
        let running_sink = Rc::clone(&running);
        let agent_event_sink = Rc::clone(&agent_events);
        let agent_session_sink = Rc::clone(&agent_sessions);
        let recorded_sink = Rc::clone(&recorded);
        let fill_sink = Rc::clone(&fill_pending);
        let registry = Arc::new(AgentTrackerRegistry::new());
        let agent_tracker = AgentTracker::with_registry(
            registry,
            AgentTrackerCallbacks {
                initial_session: None,
                shell_cwd: None,
                on_status: Box::new(|_| {}),
                on_agent: Box::new(|_| {}),
                on_announce: Box::new(|_| {}),
            },
            0,
        );
        let tracking = CommandTracking::new(
            true,
            agent_tracker,
            CommandTrackingCallbacks {
                on_command_executed: Box::new(move |command| {
                    executed_sink.borrow_mut().push(command);
                }),
                on_attention: Box::new(move |event| {
                    attention_sink.borrow_mut().push(event);
                }),
                on_agent_session_cleared: Box::new(move || {
                    agent_event_sink.borrow_mut().push("reset".to_string());
                    agent_session_sink.borrow_mut().push(None);
                }),
                on_command_recorded: Box::new(move || {
                    recorded_sink.set(recorded_sink.get() + 1);
                    fill_sink.set(false);
                }),
                on_running_command: Box::new(move |command| {
                    running_sink.borrow_mut().push(command.map(str::to_string));
                }),
            },
        );
        Self {
            tracking,
            view: Viewport {
                normal: true,
                cursor_line: 0,
                cursor_column: 0,
                viewport_line: 0,
                base_line: 0,
            },
            executed,
            attention,
            running,
            agent_events,
            agent_sessions,
            recorded,
            fill_pending,
        }
    }

    fn profile(&self) -> CommandProfile {
        CommandProfile {
            profile_id: Some("p1".to_string()),
            profile_name: Some("web".to_string()),
        }
    }

    fn typed(&mut self, command: &str, now_ms: u64) -> Option<ExecutedCommand> {
        let profile = self.profile();
        self.tracking.emit_typed_command(command, now_ms, &profile)
    }

    fn mark(&mut self, data: &str, now_ms: u64) -> Option<ExecutedCommand> {
        let profile = self.profile();
        self.tracking
            .handle_shell_mark(data, now_ms, &profile, &mut self.view)
    }
}

fn executed(command: &str, at: u64) -> ExecutedCommand {
    ExecutedCommand {
        command_text: command.to_string(),
        profile_id: Some("p1".to_string()),
        profile_name: Some("web".to_string()),
        executed_at: at,
    }
}

#[test]
fn oracle_typed_commands_trim_deduplicate_and_clear_the_last_fill() {
    let mut harness = Harness::new();
    assert!(harness.typed("", 1000).is_none());
    assert!(harness.typed(" \u{3000} ", 1000).is_none());
    assert_eq!(
        harness.typed("\u{3000} ls -la \u{3000}", 1000),
        Some(executed("ls -la", 1000))
    );
    assert!(harness.typed("ls -la", 1000).is_none());
    assert!(harness.typed("ls -la", 2499).is_none());
    assert_eq!(
        harness.typed("ls -la", 2500),
        Some(executed("ls -la", 2500))
    );

    harness.fill_pending.set(true);
    assert_eq!(harness.typed("pwd", 2500), Some(executed("pwd", 2500)));
    assert!(!harness.fill_pending.get());

    assert_eq!(
        harness.mark("E;echo reported", 2501),
        Some(executed("echo reported", 2501))
    );
    assert!(harness.typed("not restored", 2501).is_none());
    assert_eq!(
        *harness.executed.borrow(),
        vec![
            executed("ls -la", 1000),
            executed("ls -la", 2500),
            executed("pwd", 2500),
            executed("echo reported", 2501),
        ]
    );
    assert_eq!(harness.recorded.get(), 4);
}

#[test]
fn oracle_prompt_clears_the_agent_and_done_reports_command_attention() {
    let mut harness = Harness::new();
    let report = parse_agent_report("tterm-agent;claude;processing;session-a;direct;%2Ftmp")
        .expect("the agent report parses");
    assert!(harness.tracking.agent_tracker_mut().report(&report, 1500));
    assert!(harness.tracking.agent_tracker().speaks_for_agent(1500));
    harness.typed("echo reported", 1000);
    harness.mark("A", 2000);
    assert_eq!(*harness.agent_events.borrow(), vec!["reset".to_string()]);
    assert_eq!(*harness.agent_sessions.borrow(), vec![None]);
    assert!(!harness.tracking.agent_tracker().speaks_for_agent(2000));
    harness.mark("C", 2100);
    harness.mark("D;0", 2350);

    assert_eq!(
        *harness.attention.borrow(),
        vec![TerminalAttentionEvent::Command {
            command: None,
            exit_code: Some(0),
            duration_ms: 250.0,
        }]
    );
    assert!(harness.running.borrow().is_empty());
}

#[test]
fn oracle_running_command_follows_typed_and_reported_names() {
    let mut harness = Harness::new();
    harness.typed("ls -la", 1000);
    harness.mark("C", 1000);
    harness.mark("E;echo reported", 1100);
    harness.mark("D;0", 1350);

    assert_eq!(
        *harness.running.borrow(),
        vec![
            Some("ls -la".to_string()),
            Some("echo reported".to_string()),
            None,
        ]
    );
    assert_eq!(
        *harness.attention.borrow(),
        vec![TerminalAttentionEvent::Command {
            command: Some("echo reported".to_string()),
            exit_code: Some(0),
            duration_ms: 350.0,
        }]
    );
}

#[test]
fn oracle_empty_invalid_and_missing_profile_inputs_stay_empty() {
    let mut harness = Harness::new();
    assert!(harness.typed("", 1000).is_none());
    assert!(harness.typed(" \u{3000} ", 1000).is_none());
    harness.mark("B", 1000);
    harness.mark("D;x", 1000);
    assert!(harness.attention.borrow().is_empty());
    assert!(harness.agent_events.borrow().is_empty());
    assert!(harness.agent_sessions.borrow().is_empty());
    assert!(harness.executed.borrow().is_empty());

    let profile = CommandProfile::default();
    assert_eq!(
        harness
            .tracking
            .emit_typed_command(" uptime ", 1000, &profile),
        Some(ExecutedCommand {
            command_text: "uptime".to_string(),
            profile_id: None,
            profile_name: None,
            executed_at: 1000,
        })
    );
}

#[test]
fn oracle_the_command_timer_starts_from_enter_and_finishes_at_done() {
    let mut harness = Harness::new();
    harness.typed("ls -la", 1000);
    harness.tracking.command_timer_mut().start(1000);
    harness.mark("D;0", 1350);

    assert_eq!(
        *harness.running.borrow(),
        vec![Some("ls -la".to_string()), None]
    );
    assert_eq!(
        *harness.attention.borrow(),
        vec![TerminalAttentionEvent::Command {
            command: Some("ls -la".to_string()),
            exit_code: Some(0),
            duration_ms: 350.0,
        }]
    );
    assert_eq!(*harness.executed.borrow(), vec![executed("ls -la", 1000)]);
}

#[test]
fn executed_command_json_uses_camel_case_and_omits_missing_profiles() {
    let without_profile = ExecutedCommand {
        command_text: "uptime".to_string(),
        profile_id: None,
        profile_name: None,
        executed_at: 1000,
    };
    assert_eq!(
        serde_json::to_value(&without_profile).expect("the command serializes"),
        serde_json::json!({
            "commandText": "uptime",
            "executedAt": 1000,
        })
    );
    let round_trip: ExecutedCommand = serde_json::from_value(serde_json::json!({
        "commandText": "uptime",
        "profileId": "p1",
        "profileName": "web",
        "executedAt": 1000,
        "unknown": true,
    }))
    .expect("the command deserializes");
    assert_eq!(round_trip, executed("uptime", 1000));
}
