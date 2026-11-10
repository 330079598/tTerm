use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogStatus {
    /// Sessions are logged unless their connection or tab says otherwise.
    pub enabled: bool,
    pub directory: String,
    pub active_sessions: usize,
    pub recording_tab_ids: Vec<String>,
    pub total_size_bytes: u64,
    pub last_error: Option<String>,
}

/// An error from one tab or background log maintenance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogErrorPayload {
    pub tab_id: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<LogErrorPhase>,
}
/// The optional phase distinguishes failure to start a log.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogErrorPhase {
    Start,
}

use super::{EventName, EventTarget};

/// P1-03: typed session_log notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionLogEvent {
    Error(LogErrorPayload),
    Status(TerminalLogStatus),
}
impl SessionLogEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Error(_) => EventName::Static("terminal-log-error"),
            Self::Status(_) => EventName::Static("terminal-log-status"),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Any
    }
}
