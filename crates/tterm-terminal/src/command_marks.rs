//! Prompt and command positions reported by shell integration.
//!
//! The frontend implements these rules against xterm's markers, buffer lines
//! and decorations. This module keeps the decisions and leaves marker
//! registration, decoration drawing, scrolling and line lookup to the native
//! view.

use std::ops::RangeInclusive;

use crate::command_mark::CommandMark;
use crate::js_text::js_trim_end;

/// OSC codes that carry a prompt or command mark.
pub const COMMAND_MARK_OSC_CODES: [i64; 2] = [133, 633];

/// What a listener hears when the shell is back at a prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptEvent {
    /// A new prompt was marked.
    Prompt {
        /// The status of the command that ran before this prompt.
        exit_code: Option<i64>,
        /// Whether a C mark showed that the previous line ran a command.
        started_command: Option<bool>,
        /// The status carried by the D mark after the previous prompt.
        status: Option<i64>,
    },
    /// The marks stopped because they were disabled or the terminal closed.
    Ended,
}

/// Where input sent now goes, as far as the marks tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellPromptState {
    /// The shell has marked no prompt in this session.
    Unmarked,
    /// A command or full-screen program has the terminal.
    Busy,
    /// The shell is waiting at its prompt.
    Prompt,
}

/// The terminal state the command tracker reads.
///
/// The native view implements this over [`crate::snapshot::Snapshot`] and its
/// viewport. It replaces xterm's buffer and scrolling methods without changing
/// the rules that decide prompt state and command ranges.
pub trait CommandMarkViewport {
    /// Whether the normal screen is active rather than the alternate screen.
    fn is_normal_screen(&self) -> bool;
    /// The cursor's absolute grid line.
    fn cursor_line(&self) -> i64;
    /// The cursor's zero-based column.
    fn cursor_column(&self) -> usize;
    /// The top visible absolute line.
    fn viewport_line(&self) -> i64;
    /// The bottom-most possible viewport line.
    fn base_line(&self) -> i64;
    /// Scrolls so `line` is at the top when possible.
    fn scroll_to_line(&mut self, line: i64);
    /// Scrolls to the newest output.
    fn scroll_to_bottom(&mut self);
}

/// One row read from the terminal buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferLine {
    /// Whether this row continues the row above it.
    pub is_wrapped: bool,
    /// The row's text without trailing blanks.
    pub text: String,
}

/// The mark in an OSC 133 / 633 payload such as `A`, `C` or `D;1`.
pub fn parse_command_mark(data: &str) -> Option<CommandMark> {
    let mut parts = data.split(';');
    let letter = parts.next().unwrap_or_default();
    let parameter = parts.next();
    match letter {
        "A" => Some(CommandMark::Prompt),
        "C" => Some(CommandMark::Output),
        "D" => {
            let exit_code = parameter.and_then(parse_exit_code);
            Some(CommandMark::Done { exit_code })
        }
        _ => None,
    }
}

fn parse_exit_code(parameter: &str) -> Option<i64> {
    let digits = parameter.strip_prefix('-').unwrap_or(parameter);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    parameter.parse().ok()
}

/// Lines of the buffer joined as the user sees them: wrapped rows run on and
/// trailing JavaScript whitespace is removed.
pub fn join_buffer_lines(lines: &[BufferLine]) -> String {
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 && !line.is_wrapped {
            text.push('\n');
        }
        text.push_str(&line.text);
    }
    js_trim_end(&text).to_owned()
}

#[derive(Clone, Debug)]
struct CommandEntry {
    prompt_line: i64,
    output: Option<CommandOutput>,
    end: Option<CommandEnd>,
    exit_code: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
struct CommandOutput {
    line: i64,
    next_line: bool,
}

#[derive(Clone, Copy, Debug)]
struct CommandEnd {
    line: i64,
    inclusive: bool,
}

#[derive(Clone, Copy, Debug)]
struct JumpedPrompt {
    line: i64,
    viewport_line: i64,
}

/// Tracks prompts and commands from shell integration marks.
///
/// Marker disposal and decoration drawing belong to the view. The state keeps
/// absolute lines, so those view operations can happen after the decisions.
pub struct CommandMarks {
    entries: Vec<CommandEntry>,
    sends_output_marks: bool,
    sends_done_marks: bool,
    command_running: bool,
    reported_since_enter: Option<bool>,
    enabled: bool,
    jumped: Option<JumpedPrompt>,
    prompt_events: Vec<PromptEvent>,
    prompt_listener_active: bool,
    marks_prompts: bool,
    output_since_prompt: bool,
    status_since_prompt: Option<i64>,
}

impl Default for CommandMarks {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandMarks {
    /// Creates a tracker with marks enabled.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            sends_output_marks: false,
            sends_done_marks: false,
            command_running: false,
            reported_since_enter: None,
            enabled: true,
            jumped: None,
            prompt_events: Vec::new(),
            prompt_listener_active: false,
            marks_prompts: false,
            output_since_prompt: false,
            status_since_prompt: None,
        }
    }

    /// Enables or disables the tracker.
    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled == self.enabled {
            return;
        }
        self.enabled = enabled;
        if !enabled {
            self.clear();
            self.end_prompt_events();
        }
    }

    /// Starts collecting prompt events, as registering an `onPrompt` listener
    /// does in the frontend.
    pub fn listen_prompt_events(&mut self) {
        self.prompt_listener_active = true;
    }

    /// Stops collecting prompt events, as disposing that listener does.
    pub fn stop_listening_prompt_events(&mut self) {
        self.prompt_listener_active = false;
    }

    /// Takes the prompt events produced since the last call.
    pub fn take_prompt_events(&mut self) -> Vec<PromptEvent> {
        std::mem::take(&mut self.prompt_events)
    }

    /// Applies one OSC 133 or OSC 633 payload.
    pub fn handle_mark(&mut self, data: &str, view: &mut impl CommandMarkViewport) {
        if !self.enabled || !view.is_normal_screen() {
            return;
        }
        let Some(mark) = parse_command_mark(data) else {
            return;
        };
        match mark {
            CommandMark::Prompt => {
                let finished = self.current_exit_code();
                let event = PromptEvent::Prompt {
                    exit_code: finished,
                    started_command: self.sends_output_marks.then_some(self.output_since_prompt),
                    status: self.status_since_prompt,
                };
                self.output_since_prompt = false;
                self.status_since_prompt = None;
                self.marks_prompts = true;
                self.start_prompt(view);
                if self.prompt_listener_active {
                    self.prompt_events.push(event);
                }
            }
            CommandMark::Output => self.start_output(view),
            CommandMark::Done { exit_code } => {
                self.sends_done_marks = true;
                self.status_since_prompt = exit_code;
                self.finish_command(exit_code, view);
            }
        }
    }

    /// Whether the shell waits at a prompt, for shells that mark finished
    /// commands.
    pub fn is_at_prompt(&self, view: &impl CommandMarkViewport) -> bool {
        if !self.enabled || !self.sends_done_marks || !view.is_normal_screen() {
            return false;
        }
        self.entries
            .last()
            .is_some_and(|entry| entry.output.is_none() && entry.end.is_none())
    }

    /// The prompt state known without needing a still-live prompt marker.
    pub fn prompt_state(&self, view: &impl CommandMarkViewport) -> ShellPromptState {
        if !self.enabled || !self.marks_prompts {
            return ShellPromptState::Unmarked;
        }
        if !view.is_normal_screen() || self.command_running {
            return ShellPromptState::Busy;
        }
        ShellPromptState::Prompt
    }

    /// Whether input goes to a program rather than the shell's line editor.
    pub fn is_running_program(&self, view: &impl CommandMarkViewport) -> bool {
        if !view.is_normal_screen() {
            return true;
        }
        self.enabled && self.command_running
    }

    /// Notes that the shell reported its directory.
    pub fn note_cwd_report(&mut self) {
        self.reported_since_enter = Some(true);
    }

    /// Whether Enter was pressed and no directory report has followed.
    pub fn awaits_prompt(&self) -> bool {
        self.reported_since_enter == Some(false)
    }

    /// Handles Enter at the prompt for shells that send no C mark.
    pub fn handle_enter(&mut self, view: &mut impl CommandMarkViewport) -> bool {
        if self.reported_since_enter == Some(true) {
            self.reported_since_enter = Some(false);
        }
        if !self.enabled || self.sends_output_marks || !view.is_normal_screen() {
            return false;
        }
        let Some(entry) = self.entries.last_mut() else {
            return false;
        };
        if entry.output.is_some() || entry.end.is_some() {
            return false;
        }
        entry.output = Some(CommandOutput {
            line: view.cursor_line(),
            next_line: true,
        });
        self.command_running = true;
        true
    }

    /// Scrolls the previous prompt to the top. Returns false when no prompt is
    /// known.
    pub fn scroll_to_previous_prompt(&mut self, view: &mut impl CommandMarkViewport) -> bool {
        let prompts: Vec<i64> = self.entries.iter().map(|entry| entry.prompt_line).collect();
        if prompts.is_empty() {
            return false;
        }
        let jumped_line = self.jumped_prompt_line(view);
        let mut reference = jumped_line.unwrap_or_else(|| view.viewport_line());
        if jumped_line.is_none()
            && view.viewport_line() >= view.base_line()
            && let Some(last) = self.entries.last()
        {
            reference = if last.output.is_some() && last.end.is_none() {
                i64::MAX
            } else {
                last.prompt_line
            };
        }
        if let Some(target) = prompts.into_iter().rev().find(|line| *line < reference) {
            self.jump_to(target, view);
        }
        true
    }

    /// Scrolls the next prompt to the top, or to the bottom past the last one.
    pub fn scroll_to_next_prompt(&mut self, view: &mut impl CommandMarkViewport) -> bool {
        let prompts: Vec<i64> = self.entries.iter().map(|entry| entry.prompt_line).collect();
        if prompts.is_empty() {
            return false;
        }
        let reference = self
            .jumped_prompt_line(view)
            .unwrap_or_else(|| view.viewport_line());
        let target = prompts.into_iter().find(|line| *line > reference);
        if let Some(target) = target.filter(|line| *line < view.base_line()) {
            self.jump_to(target, view);
        } else {
            self.jumped = None;
            view.scroll_to_bottom();
        }
        true
    }

    /// The absolute line range of the last finished command's output.
    pub fn last_output_range(&self) -> Option<RangeInclusive<i64>> {
        for entry in self.entries.iter().rev() {
            let (Some(output), Some(end)) = (entry.output, entry.end) else {
                continue;
            };
            if entry.exit_code.is_none() {
                continue;
            }
            let start = output.line + i64::from(output.next_line);
            let finish = end.line - i64::from(!end.inclusive);
            return Some(start..=finish);
        }
        None
    }

    /// Text of the last finished command's output.
    ///
    /// The caller supplies buffer rows because the native terminal model
    /// copies only the visible area into a snapshot. A view can answer from
    /// its scrollback source or from the terminal selection API.
    pub fn last_output_text(
        &self,
        mut line_at: impl FnMut(i64) -> Option<BufferLine>,
    ) -> Option<String> {
        let range = self.last_output_range()?;
        let mut lines = Vec::new();
        for line in *range.start()..=*range.end() {
            if let Some(line) = line_at(line) {
                lines.push(line);
            }
        }
        Some(join_buffer_lines(&lines))
    }

    /// Whether the last finished command's output can be selected.
    pub fn select_last_output(&self) -> bool {
        self.last_output_range()
            .is_some_and(|range| range.end() >= range.start())
    }

    /// Stops tracking and tells listeners the marks ended.
    pub fn dispose(&mut self) {
        self.clear();
        self.end_prompt_events();
    }

    fn end_prompt_events(&mut self) {
        if !self.prompt_listener_active {
            return;
        }
        self.prompt_listener_active = false;
        self.prompt_events.push(PromptEvent::Ended);
    }

    fn current_exit_code(&self) -> Option<i64> {
        self.entries.last().and_then(|entry| entry.exit_code)
    }

    fn jump_to(&mut self, line: i64, view: &mut impl CommandMarkViewport) {
        view.scroll_to_line(line);
        self.jumped = Some(JumpedPrompt {
            line,
            viewport_line: view.viewport_line(),
        });
    }

    fn jumped_prompt_line(&mut self, view: &impl CommandMarkViewport) -> Option<i64> {
        let jumped = self.jumped?;
        if jumped.viewport_line != view.viewport_line() {
            self.jumped = None;
            return None;
        }
        Some(jumped.line)
    }

    fn start_prompt(&mut self, view: &mut impl CommandMarkViewport) {
        self.command_running = false;
        if let Some(index) = self.entries.len().checked_sub(1)
            && self.entries[index].end.is_none()
        {
            self.end_entry(index, view);
        }
        self.entries.push(CommandEntry {
            prompt_line: view.cursor_line(),
            output: None,
            end: None,
            exit_code: None,
        });
    }

    fn start_output(&mut self, view: &mut impl CommandMarkViewport) {
        self.sends_output_marks = true;
        self.output_since_prompt = true;
        self.command_running = true;
        let Some(entry) = self.entries.last_mut() else {
            return;
        };
        if entry.end.is_some() || entry.output.is_some_and(|output| !output.next_line) {
            return;
        }
        entry.output = Some(CommandOutput {
            line: view.cursor_line(),
            next_line: false,
        });
    }

    fn finish_command(&mut self, exit_code: Option<i64>, view: &mut impl CommandMarkViewport) {
        self.command_running = false;
        let Some(index) = self.entries.len().checked_sub(1) else {
            return;
        };
        if self.entries[index].end.is_some() {
            return;
        }
        self.end_entry(index, view);
        if self.entries[index].output.is_none() || exit_code.is_none() {
            return;
        }
        self.entries[index].exit_code = exit_code;
    }

    fn end_entry(&mut self, index: usize, view: &impl CommandMarkViewport) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.end = Some(CommandEnd {
                line: view.cursor_line(),
                inclusive: view.cursor_column() > 0,
            });
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.jumped = None;
        self.command_running = false;
    }
}

#[cfg(test)]
mod tests;
