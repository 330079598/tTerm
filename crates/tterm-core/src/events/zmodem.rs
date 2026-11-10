use serde::Serialize;

/// Identifies a transfer and the file being sent or received.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStartPayload {
    /// The identifier shared by all updates for this transfer.
    pub transfer_id: String,
    /// Whether this file is being uploaded or downloaded.
    pub direction: &'static str,
    /// The file name displayed in the transfer list.
    pub file_name: String,
    /// The expected file size in bytes, or zero when unknown.
    pub file_size: u64,
    /// The local source or destination path.
    pub local_path: String,
    /// The sender gave no size, as XMODEM never does: `file_size` is 0 and
    /// the list shows what has arrived without a total.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub size_unknown: bool,
}

/// Reports the bytes transferred and the current speed in bytes per second.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressPayload {
    /// The identifier of the transfer being updated.
    pub transfer_id: String,
    /// The number of bytes transferred so far.
    pub transferred: u64,
    /// The current transfer speed in bytes per second.
    pub speed: u64,
}

/// Reports whether a transfer succeeded, was cancelled or ended with an error.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCompletePayload {
    /// The identifier of the transfer that ended.
    pub transfer_id: String,
    /// Whether the transfer completed successfully.
    pub success: bool,
    /// Whether the transfer ended because it was cancelled.
    pub cancelled: bool,
    /// The failure message, when one is available.
    pub error: Option<String>,
}

/// A typed transfer update shared by ZMODEM and serial file transfers.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferEvent {
    /// A file transfer has started.
    Start(TransferStartPayload),
    /// A running transfer has updated its byte count and speed.
    Progress(TransferProgressPayload),
    /// A file transfer has finished, failed or been cancelled.
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
    /// The remote receiver is ready for the user to choose files to send.
    SendRequested { tab_id: String },
    /// Reports a tab's ZMODEM transfer lifecycle update.
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
