use serde::Serialize;

/// A local shell directory update scoped to the current session nonce.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CwdPayload {
    pub session_nonce: u32,
    pub cwd: String,
}

use super::{EventName, EventTarget};

/// P1-03: typed terminal notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalEvent {
    /// Reports the local shell's working directory for a tab and session nonce.
    Cwd { tab_id: String, payload: CwdPayload },
}
impl TerminalEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Cwd { tab_id, .. } => EventName::TabScoped {
                prefix: "pty-cwd",
                tab_id: tab_id.clone(),
            },
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Any
    }
}
