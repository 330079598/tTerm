use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStartPayload {
    pub transfer_id: String,
    pub direction: &'static str,
    pub file_name: String,
    pub file_size: u64,
    pub local_path: String,
    /// The sender gave no size, as XMODEM never does: `file_size` is 0 and
    /// the list shows what has arrived without a total.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub size_unknown: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressPayload {
    pub transfer_id: String,
    pub transferred: u64,
    pub speed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCompletePayload {
    pub transfer_id: String,
    pub success: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}

/// A typed transfer update shared by ZMODEM and serial file transfers.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferEvent {
    Start(TransferStartPayload),
    Progress(TransferProgressPayload),
    Complete(TransferCompletePayload),
}
impl TransferEvent {
    /// Preserves the existing per-tab transfer event names.
    pub fn name(&self, tab_id: &str) -> EventName {
        let prefix = match self {
            Self::Start(_) => "zmodem-transfer-start",
            Self::Progress(_) => "zmodem-transfer-progress",
            Self::Complete(_) => "zmodem-transfer-complete",
        };
        EventName::TabScoped {
            prefix,
            tab_id: tab_id.to_string(),
        }
    }
}
/// Receives a typed update on the thread that drives a transfer.
pub type TransferEmitter = Box<dyn Fn(TransferEvent) + Send>;

use super::{EventName, EventTarget};

/// P1-03: typed zmodem notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum ZmodemEvent {
    SendRequested {
        tab_id: String,
    },
    Transfer {
        tab_id: String,
        event: TransferEvent,
    },
}
impl ZmodemEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::SendRequested { tab_id } => EventName::TabScoped {
                prefix: "zmodem-send-requested",
                tab_id: tab_id.clone(),
            },
            Self::Transfer { tab_id, event } => event.name(tab_id),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Any
    }
}
