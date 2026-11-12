//! Types a login script into a shell one command at a time.

use tterm_terminal::js_text::{js_trim, js_trim_end};

/// Quiet time after a line that looks like a shell prompt before the next
/// command is typed.
pub const LOGIN_SCRIPT_PROMPT_IDLE_MS: u64 = 250;
/// Quiet time after any other output.
pub const LOGIN_SCRIPT_FALLBACK_IDLE_MS: u64 = 1500;

/// The commands of a login script: one per line, blank lines skipped.
pub fn parse_login_script(script: Option<&str>) -> Vec<String> {
    let normalized = script
        .unwrap_or_default()
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    normalized
        .split('\n')
        .map(js_trim_end)
        .filter(|line| !js_trim(line).is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether the text before the cursor looks like a shell waiting for a
/// command.
pub fn looks_like_shell_prompt(line: &str) -> bool {
    js_trim_end(line)
        .chars()
        .last()
        .is_some_and(|character| "$#%>❯➜λ»".contains(character))
}

/// The callbacks and limits a login-script runner needs.
pub struct LoginScriptRunnerOptions {
    /// The commands to type.
    pub lines: Vec<String>,
    /// Types one command; the caller adds the line ending.
    pub send: Box<dyn FnMut(&str)>,
    /// Reads the text before the cursor when the output goes quiet.
    pub read_cursor_line: Box<dyn Fn() -> String>,
    /// Whether a password prompt waits for the user.
    pub is_at_password_prompt: Box<dyn Fn() -> bool>,
    /// Overrides the quiet time after a usual prompt.
    pub prompt_idle_ms: Option<u64>,
    /// Overrides the quiet time after other output.
    pub fallback_idle_ms: Option<u64>,
}

/// Types a login script into a shell one command at a time.
///
/// The runner owns no timer. The adapter asks for [`LoginScriptRunner::next_deadline`]
/// and calls [`LoginScriptRunner::fire_due`] when that time arrives, which
/// keeps the behaviour testable without fake clocks.
pub struct LoginScriptRunner {
    options: LoginScriptRunnerOptions,
    next_index: usize,
    deadline: Option<u64>,
    stopped: bool,
}

impl LoginScriptRunner {
    /// Starts the runner; `now_ms` is the current clock value.
    pub fn new(options: LoginScriptRunnerOptions, now_ms: u64) -> Self {
        let stopped = options.lines.is_empty();
        let mut runner = Self {
            options,
            next_index: 0,
            deadline: None,
            stopped,
        };
        if !stopped {
            runner.note_output(now_ms);
        }
        runner
    }

    /// Whether the runner has sent every line or was stopped.
    pub fn done(&self) -> bool {
        self.stopped
    }

    /// The clock value at which [`LoginScriptRunner::fire_due`] should run,
    /// or `None` when no timer is armed.
    pub fn next_deadline(&self) -> Option<u64> {
        self.deadline
    }

    /// Call after each output chunk is on screen; `now_ms` is the clock value.
    pub fn note_output(&mut self, now_ms: u64) {
        if self.stopped {
            return;
        }
        let delay = if looks_like_shell_prompt(&(self.options.read_cursor_line)()) {
            self.options
                .prompt_idle_ms
                .unwrap_or(LOGIN_SCRIPT_PROMPT_IDLE_MS)
        } else {
            self.options
                .fallback_idle_ms
                .unwrap_or(LOGIN_SCRIPT_FALLBACK_IDLE_MS)
        };
        self.deadline = Some(now_ms + delay);
    }

    /// Stops the runner and cancels any timer.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.deadline = None;
    }

    /// Runs the armed timer if `now_ms` has reached its deadline.
    pub fn fire_due(&mut self, now_ms: u64) {
        if self.stopped {
            return;
        }
        let Some(deadline) = self.deadline else {
            return;
        };
        if now_ms < deadline {
            return;
        }
        self.deadline = None;
        // Wait for the output that follows the user's answer.
        if (self.options.is_at_password_prompt)() {
            return;
        }
        let Some(line) = self.options.lines.get(self.next_index).cloned() else {
            self.stop();
            return;
        };
        self.next_index += 1;
        if self.next_index >= self.options.lines.len() {
            self.stop();
        }
        (self.options.send)(&line);
    }
}

#[cfg(test)]
mod tests;
