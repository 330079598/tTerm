//! Bounded scanning for shell-integration prompt marks in terminal output.
//!
//! The Tauri client asks xterm's current mouse mode from an OSC handler when
//! a prompt mark arrives. The native model does not expose parser handlers to
//! the session, so the parser splits the output stream at completed top-level
//! OSC 133/633 type A marks and asks the real model at each one.
//!
//! The scanner must never become a second, unbounded parser. It shares bounded
//! control-string framing with other output consumers, keeps only a short
//! first field for prompt marks, and returns byte offsets rather than decoded
//! strings. Its state survives arbitrary chunk splits, including a split
//! between `ESC` and the `\` that completes a string terminator.

use crate::command_mark::CommandMark;
use crate::command_marks::parse_command_mark;
use crate::control_strings::{CompletedControlString, ControlStringFramer};

/// The exact sequence the Tauri guard writes to close stale mouse reporting.
pub const MOUSE_REPORTING_OFF: &[u8] = b"\x1b[?9l\x1b[?1000l\x1b[?1001l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";

/// Scans decoded output for completed OSC 133/633 prompt marks.
#[derive(Debug, Default)]
pub struct MousePromptScanner {
    framer: ControlStringFramer,
}

impl MousePromptScanner {
    /// Scans one chunk and returns the offsets just past each completed
    /// top-level prompt terminator in this chunk.
    ///
    /// An offset is always after the complete BEL or ST terminator. A split
    /// between `ESC` and `\` is never reported, because that would leave the
    /// model half way through a string terminator.
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<usize> {
        let mut offsets = Vec::new();
        self.framer.scan(bytes, |event| {
            Self::record(event, &mut offsets);
        });
        offsets
    }

    fn record(event: CompletedControlString<'_>, offsets: &mut Vec<usize>) {
        if matches!(event.code, Some(133 | 633))
            && event.first_field_valid
            && std::str::from_utf8(event.first_field)
                .ok()
                .and_then(parse_command_mark)
                == Some(CommandMark::Prompt)
        {
            offsets.push(event.end);
        }
    }
}

#[cfg(test)]
mod tests;
