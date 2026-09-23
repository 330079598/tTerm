//! Shared event-payload shapes for ZMODEM transfer notifications
//! (`zmodem-transfer-start-{tabId}` etc.), used by both the receive driver
//! (`session.rs`) and the send driver (`send.rs`) so the frontend
//! (`useZmodemTransfers.ts`) handles both directions identically.

use serde::Serialize;

/// Notifies the frontend of one `kind` of event (e.g. `"transfer-start"`)
/// for a specific tab; the full event name (`zmodem-{kind}-{tabId}`) is
/// assembled by whoever constructs this closure, since that's also where
/// the tab id is already known. Boxed so `ZmodemReceiveDriver`/the send
/// session aren't tied to a concrete `AppHandle` — see `session.rs`'s doc
/// comment for why that matters for testing.
pub type EmitFn = Box<dyn Fn(&str, serde_json::Value) + Send>;

pub fn emit<T: Serialize>(emit_fn: &EmitFn, kind: &str, payload: T) {
    if let Ok(value) = serde_json::to_value(payload) {
        emit_fn(kind, value);
    }
}

/// A fresh id for one transfer-manager row (`TransferTask.id` on the
/// frontend). Shared by the receive and send drivers so both mint ids the
/// same way.
pub fn new_transfer_id() -> String {
    format!("zmodem-{}", uuid::Uuid::new_v4())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStartPayload {
    pub transfer_id: String,
    pub direction: &'static str,
    pub file_name: String,
    pub file_size: u64,
    pub local_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressPayload {
    pub transfer_id: String,
    pub transferred: u64,
    pub speed: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCompletePayload {
    pub transfer_id: String,
    pub success: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}
