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

/// The most payload bytes kept for one command mark.
const MAX_COMMAND_MARK_PAYLOAD_BYTES: usize = 4 * 1024;

/// The OSC codes command marks use, as the bounded framer stores them.
const COMMAND_MARK_OSC_CODES: [u16; 2] = [
    crate::command_marks::COMMAND_MARK_OSC_CODES[0] as u16,
    crate::command_marks::COMMAND_MARK_OSC_CODES[1] as u16,
];

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

/// One completed control string and what the session or the model made of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScannedControl {
    /// The byte offset just past the terminator.
    pub(crate) end: usize,
    /// The session report this control string carries, when it carries one.
    pub(crate) report: Option<SessionReport>,
    /// The command mark this control string carries, when it carries one.
    pub(crate) mark: Option<CommandMark>,
}

/// A completed command mark and the byte offset just past its terminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommandMarkEvent {
    /// The byte offset just past the terminator.
    pub(crate) end: usize,
    /// The parsed mark.
    pub(crate) mark: CommandMark,
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

    /// Creates the same scanner configured for the model's command marks.
    ///
    /// The framing state machine is shared with [`Self::new`], so both
    /// scanners split chunks and control strings at identical boundaries.
    pub(crate) fn for_command_marks() -> Self {
        Self {
            framer: ControlStringFramer::with_payload_codes(
                &COMMAND_MARK_OSC_CODES,
                MAX_COMMAND_MARK_PAYLOAD_BYTES,
            ),
        }
    }

    /// Scans one chunk and returns every complete report it contains.
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<SessionReport> {
        self.scan_controls(bytes)
            .into_iter()
            .filter_map(|control| control.report)
            .collect()
    }

    /// Scans one chunk and returns the model's complete command marks.
    pub(crate) fn scan_command_marks(&mut self, bytes: &[u8]) -> Vec<CommandMarkEvent> {
        self.scan_controls(bytes)
            .into_iter()
            .filter_map(|control| {
                control.mark.map(|mark| CommandMarkEvent {
                    end: control.end,
                    mark,
                })
            })
            .collect()
    }

    /// Scans one chunk and keeps both the report and the mark of each control.
    pub(crate) fn scan_controls(&mut self, bytes: &[u8]) -> Vec<ScannedControl> {
        let mut controls = Vec::new();
        self.framer.scan(bytes, |event| {
            Self::record(event, &mut controls);
        });
        controls
    }

    fn record(event: CompletedControlString<'_>, controls: &mut Vec<ScannedControl>) {
        if !event.payload_valid {
            return;
        }
        let Some(code) = event.code else {
            return;
        };
        let Ok(data) = std::str::from_utf8(event.payload) else {
            return;
        };
        let mut report = None;
        let mut mark = None;
        match code {
            7 | 9 => {
                if let Some(cwd) = parse_cwd_report(i64::from(code), data) {
                    report = Some(SessionReport::Cwd(cwd));
                }
            }
            133 | 633 => {
                mark = parse_command_mark(data);
                match mark {
                    Some(CommandMark::Prompt) => report = Some(SessionReport::Prompt),
                    Some(CommandMark::Output) => report = Some(SessionReport::CommandStarted),
                    Some(CommandMark::Done { .. }) | None => {}
                }
            }
            777 if data.starts_with("tterm-agent;") => {
                report = Some(SessionReport::AgentReport(data.to_owned()));
            }
            777 => {}
            _ => {}
        }
        if report.is_some() || mark.is_some() {
            controls.push(ScannedControl {
                end: event.end,
                report,
                mark,
            });
        }
    }
}

#[cfg(test)]
mod tests;
