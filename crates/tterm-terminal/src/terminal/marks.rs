//! Runtime control and bookkeeping for shell-integration marks.

use super::State;
use super::Terminal;
use crate::command_marks::overview::CommandMarkState;
use crate::history_release::MarkChanges;
use crate::session_reports::CommandMarkEvent;

impl Terminal {
    /// Turns shell-integration marks on or off at runtime.
    ///
    /// Turning them off drops every position already collected, so the
    /// overview ruler and command navigation cannot keep stale entries. New
    /// output while they are off creates no positions; turning them back on
    /// starts collecting from the next prompt. An unchanged value is a no-op.
    pub fn set_command_marks_enabled(&self, enabled: bool) {
        let mut state = self.state.lock();
        if state.command_marks_enabled == enabled {
            return;
        }
        state.command_marks_enabled = enabled;
        if !enabled {
            state.command_marks = CommandMarkState::new();
        }
    }

    /// Whether shell-integration marks are currently collected.
    pub fn command_marks_enabled(&self) -> bool {
        self.state.lock().command_marks_enabled
    }
}

impl State {
    /// Scans a batch for shell-integration marks when collection is enabled.
    pub(super) fn scan_command_marks(&mut self, bytes: &[u8]) -> Vec<CommandMarkEvent> {
        if self.command_marks_enabled {
            self.command_marks.scan(bytes)
        } else {
            Vec::new()
        }
    }
}

/// Applies the history changes the parser made during one slice.
pub(super) fn apply_mark_changes(command_marks: &mut CommandMarkState, changes: MarkChanges) {
    if changes.dispose {
        command_marks.dispose_markers();
    } else if changes.shift > 0 {
        command_marks.trim_scrollback(changes.shift);
    }
}
