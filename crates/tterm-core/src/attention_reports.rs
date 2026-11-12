//! Attention requests and agent reports carried by terminal OSC sequences.
//!
//! The frontend registers xterm OSC handlers and a bell listener. Native
//! session events call this state machine instead; whether the terminal is
//! local or remote does not change these rules.

use crate::agent_status::{AgentReportState, AgentResumeSession, AgentTracker};
use crate::terminal_notifications::{
    KittyNotificationAssembler, TerminalAttentionEvent, TerminalNotificationRequest,
    parse_osc9_notification, parse_osc777_notification,
};

/// The OSC code for agent reports and urxvt notifications.
const OSC_777: i64 = 777;
/// The OSC code for iTerm2 notifications.
const OSC_9: i64 = 9;
/// The OSC code for kitty notifications.
const OSC_99: i64 = 99;

/// Follows attention requests from one terminal.
///
/// The caller owns the terminal event registration and the agent tracker. The
/// tracker uses an explicit clock so a frontend adapter can pass its own
/// current time.
pub struct AttentionReports {
    kitty: KittyNotificationAssembler,
    on_attention: Box<dyn FnMut(TerminalAttentionEvent)>,
    on_agent_session: Box<dyn FnMut(Option<AgentResumeSession>)>,
}

impl AttentionReports {
    /// Creates a report handler with its adapter callbacks.
    pub fn new(
        on_attention: impl FnMut(TerminalAttentionEvent) + 'static,
        on_agent_session: impl FnMut(Option<AgentResumeSession>) + 'static,
    ) -> Self {
        Self {
            kitty: KittyNotificationAssembler::new(),
            on_attention: Box::new(on_attention),
            on_agent_session: Box::new(on_agent_session),
        }
    }

    /// Handles one OSC payload.
    ///
    /// The caller should pass the payload to the directory report handler as
    /// well; the frontend's handlers all return false for the same reason.
    pub fn handle_osc(&mut self, tracker: &mut AgentTracker, code: i64, data: &str, now_ms: u64) {
        if code == OSC_777
            && let Some(report) = crate::agent_status::parse_agent_report(data)
        {
            if tracker.report(&report, now_ms) {
                if let Some(session) = tracker.resume_session(&report) {
                    (self.on_agent_session)(Some(session));
                } else if matches!(
                    report.state,
                    AgentReportState::Ended | AgentReportState::Idle
                ) {
                    (self.on_agent_session)(None);
                }
            }
            return;
        }
        if tracker.speaks_for_agent(now_ms) {
            return;
        }
        let request = match code {
            OSC_9 => parse_osc9_notification(data),
            OSC_777 => parse_osc777_notification(data),
            OSC_99 => self.kitty.handle(data),
            _ => None,
        };
        if let Some(request) = request {
            (self.on_attention)(attention_request(request));
        }
    }

    /// Handles the terminal bell.
    pub fn handle_bell(&mut self) {
        (self.on_attention)(TerminalAttentionEvent::Bell);
    }

    /// Notes Enter typed in the terminal, which may claim the next new
    /// session.
    pub fn submit(&mut self, tracker: &mut AgentTracker, now_ms: u64) {
        tracker.submit(now_ms);
    }
}

fn attention_request(request: TerminalNotificationRequest) -> TerminalAttentionEvent {
    TerminalAttentionEvent::Request {
        title: request.title,
        body: request.body,
    }
}

#[cfg(test)]
mod tests;
