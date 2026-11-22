//! Bounded recognition of shell-integration and agent reports.
//!
//! The model keeps the escape sequences out of the screen, but the session
//! needs three of them: a prompt mark, an AI agent's report, and the shell's
//! working directory. This scanner shares the bounded control-string framing
//! with the OSC 52 and mouse scanners, so a chunk split, an overlong payload,
//! or an invalid code never becomes a second parser.

use crate::command_mark::CommandMark;
use crate::command_marks::parse_command_mark;
use crate::control_strings::{CompletedControlString, ControlStringFramer};
use crate::terminal_cwd::parse_cwd_report;

/// The OSC codes this scanner keeps.
const REPORT_OSC_CODES: [u16; 5] = [7, 9, 133, 633, 777];

/// The most payload bytes kept for one report.
///
/// Agent reports carry a percent-encoded directory, which the core parser
/// bounds at 16 KiB after decoding. Keeping a little more here lets an
/// invalid report be rejected as a whole instead of treated as a valid
/// prefix.
const MAX_REPORT_PAYLOAD_BYTES: usize = 32 * 1024;

/// A report the session needs from the output stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionReport {
    /// A complete OSC 777 payload that starts with `tterm-agent;`.
    AgentReport(String),
    /// An OSC 133 or OSC 633 prompt mark.
    Prompt,
    /// An OSC 133 or OSC 633 output mark; the shell is no longer at its prompt.
    CommandStarted,
    /// A working-directory report from OSC 7 or OSC 9;9.
    Cwd(String),
}

/// Recognizes complete reports across arbitrary output chunk boundaries.
#[derive(Debug, Default)]
pub struct SessionReportScanner {
    framer: ControlStringFramer,
}

impl SessionReportScanner {
    /// Creates a scanner that keeps report payloads up to the shared bound.
    pub fn new() -> Self {
        Self {
            framer: ControlStringFramer::with_payload_codes(
                &REPORT_OSC_CODES,
                MAX_REPORT_PAYLOAD_BYTES,
            ),
        }
    }

    /// Scans one chunk and returns every complete report it contains.
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<SessionReport> {
        let mut reports = Vec::new();
        self.framer.scan(bytes, |event| {
            Self::record(event, &mut reports);
        });
        reports
    }

    fn record(event: CompletedControlString<'_>, reports: &mut Vec<SessionReport>) {
        if !event.payload_valid {
            return;
        }
        let Some(code) = event.code else {
            return;
        };
        let Ok(data) = std::str::from_utf8(event.payload) else {
            return;
        };
        match code {
            7 | 9 => {
                if let Some(cwd) = parse_cwd_report(i64::from(code), data) {
                    reports.push(SessionReport::Cwd(cwd));
                }
            }
            133 | 633 => match parse_command_mark(data) {
                Some(CommandMark::Prompt) => reports.push(SessionReport::Prompt),
                Some(CommandMark::Output) => reports.push(SessionReport::CommandStarted),
                Some(CommandMark::Done { .. }) | None => {}
            },
            777 if data.starts_with("tterm-agent;") => {
                reports.push(SessionReport::AgentReport(data.to_owned()));
            }
            777 => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
