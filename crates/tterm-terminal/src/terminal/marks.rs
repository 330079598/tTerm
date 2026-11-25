//! Bookkeeping for shell-integration marks.

use crate::command_marks::overview::CommandMarkState;
use crate::history_release::MarkChanges;

/// Applies the history changes the parser made during one slice.
pub(super) fn apply_mark_changes(command_marks: &mut CommandMarkState, changes: MarkChanges) {
    if changes.dispose {
        command_marks.dispose_markers();
    } else if changes.shift > 0 {
        command_marks.trim_scrollback(changes.shift);
    }
}
