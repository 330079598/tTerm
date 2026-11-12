//! Command lines, timing and shell-integration marks for one terminal.
//!
//! The frontend wires this state to xterm, the PTY and several workspace
//! stores. The native adapter does the same wiring around this module; the
//! deduplication and shell-mark rules stay here.

use serde::{Deserialize, Serialize};
use tterm_terminal::command_mark::CommandMark;
use tterm_terminal::command_marks::{CommandMarkViewport, CommandMarks, parse_command_mark};
use tterm_terminal::js_text::js_trim;
use tterm_terminal::terminal_command_capture::parse_shell_integration_command;

use crate::agent_status::AgentTracker;
use crate::terminal_notifications::{CommandTimer, TerminalAttentionEvent};

/// A command line sent to the command history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutedCommand {
    /// The command text.
    pub command_text: String,
    /// The saved profile's identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    /// The saved profile's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    /// The time the command was recorded.
    pub executed_at: u64,
}

/// The profile fields copied onto a recorded command.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandProfile {
    /// The saved profile's identifier.
    pub profile_id: Option<String>,
    /// The saved profile's name.
    pub profile_name: Option<String>,
}

/// Receives the command a tab should show as running, or none when it ends.
pub type RunningCommandCallback = Box<dyn FnMut(Option<&str>)>;

/// Callbacks the terminal session adapter supplies.
pub struct CommandTrackingCallbacks {
    /// Receives a command line for history, before the shell reports the line.
    pub on_command_executed: Box<dyn FnMut(ExecutedCommand)>,
    /// Receives finished commands and other attention events.
    pub on_attention: Box<dyn FnMut(TerminalAttentionEvent)>,
    /// Clears the tab's agent session when a new prompt appears.
    pub on_agent_session_cleared: Box<dyn FnMut()>,
    /// Clears the last password fill when a new command starts.
    pub on_command_recorded: Box<dyn FnMut()>,
    /// Receives the command a tab should show as running.
    pub on_running_command: RunningCommandCallback,
}

/// Follows typed and reported command lines for one session.
pub struct CommandTracking {
    agent_tracker: AgentTracker,
    command_timer: CommandTimer,
    command_marks: CommandMarks,
    last_emitted_command: Option<(String, u64)>,
    shell_reports_commands: bool,
    callbacks: TrackingCallbacks,
}

struct TrackingCallbacks {
    on_command_executed: Box<dyn FnMut(ExecutedCommand)>,
    on_attention: Box<dyn FnMut(TerminalAttentionEvent)>,
    on_agent_session_cleared: Box<dyn FnMut()>,
    on_command_recorded: Box<dyn FnMut()>,
}

impl CommandTracking {
    /// Creates the tracker with the shared agent tracker and the configured
    /// command-mark setting.
    pub fn new(
        enabled: bool,
        agent_tracker: AgentTracker,
        callbacks: CommandTrackingCallbacks,
    ) -> Self {
        let CommandTrackingCallbacks {
            on_command_executed,
            on_attention,
            on_agent_session_cleared,
            on_command_recorded,
            on_running_command,
        } = callbacks;
        let command_timer = CommandTimer::with_on_running(on_running_command);
        Self {
            agent_tracker,
            command_timer,
            command_marks: {
                let mut marks = CommandMarks::new();
                marks.set_enabled(enabled);
                marks
            },
            last_emitted_command: None,
            shell_reports_commands: false,
            callbacks: TrackingCallbacks {
                on_command_executed,
                on_attention,
                on_agent_session_cleared,
                on_command_recorded,
            },
        }
    }

    /// The agent tracker shared with the attention report handlers.
    pub fn agent_tracker(&self) -> &AgentTracker {
        &self.agent_tracker
    }

    /// The agent tracker shared with the attention report handlers.
    pub fn agent_tracker_mut(&mut self) -> &mut AgentTracker {
        &mut self.agent_tracker
    }

    /// The prompt and command state shared with the terminal view.
    pub fn command_marks(&self) -> &CommandMarks {
        &self.command_marks
    }

    /// The mutable prompt and command state shared with the terminal view.
    pub fn command_marks_mut(&mut self) -> &mut CommandMarks {
        &mut self.command_marks
    }

    /// The command timer shared with the input and PTY adapters.
    pub fn command_timer(&self) -> &CommandTimer {
        &self.command_timer
    }

    /// The command timer shared with the input and PTY adapters.
    pub fn command_timer_mut(&mut self) -> &mut CommandTimer {
        &mut self.command_timer
    }

    /// Handles an OSC 133 or OSC 633 payload.
    pub fn handle_shell_mark(
        &mut self,
        data: &str,
        now_ms: u64,
        profile: &CommandProfile,
        view: &mut impl CommandMarkViewport,
    ) -> Option<ExecutedCommand> {
        self.command_marks.handle_mark(data, view);
        self.time_command(data, now_ms);
        let command = parse_shell_integration_command(data)?;
        self.shell_reports_commands = true;
        self.command_timer.set_command(&command);
        self.record_command(&command, now_ms, profile)
    }

    /// Records a line typed at the prompt, unless the shell reports its own.
    pub fn emit_typed_command(
        &mut self,
        command_text: &str,
        now_ms: u64,
        profile: &CommandProfile,
    ) -> Option<ExecutedCommand> {
        let normalized = js_trim(command_text);
        if normalized.is_empty() || self.shell_reports_commands {
            return None;
        }
        self.command_timer.set_typed_command(normalized);
        self.record_command(normalized, now_ms, profile)
    }

    fn time_command(&mut self, data: &str, now_ms: u64) {
        let Some(mark) = parse_command_mark(data) else {
            return;
        };
        if mark == CommandMark::Prompt {
            self.agent_tracker.reset();
            (self.callbacks.on_agent_session_cleared)();
        }
        let finished = self.command_timer.mark(mark, now_ms as i64);
        if let Some(finished) = finished {
            (self.callbacks.on_attention)(TerminalAttentionEvent::Command {
                command: finished.command,
                exit_code: finished.exit_code,
                duration_ms: finished.duration_ms as f64,
            });
        }
    }

    fn record_command(
        &mut self,
        command: &str,
        now_ms: u64,
        profile: &CommandProfile,
    ) -> Option<ExecutedCommand> {
        if self
            .last_emitted_command
            .as_ref()
            .is_some_and(|(text, at)| text == command && now_ms.saturating_sub(*at) < 1500)
        {
            return None;
        }
        (self.callbacks.on_command_recorded)();
        self.last_emitted_command = Some((command.to_string(), now_ms));
        let executed = ExecutedCommand {
            command_text: command.to_string(),
            profile_id: profile.profile_id.clone(),
            profile_name: profile.profile_name.clone(),
            executed_at: now_ms,
        };
        (self.callbacks.on_command_executed)(executed.clone());
        Some(executed)
    }
}

#[cfg(test)]
mod tests;
