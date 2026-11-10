//! Shared event-payload shapes for ZMODEM transfer notifications
//! (`zmodem-transfer-start-{tabId}` etc.), used by both the receive driver
//! (`session.rs`) and the send driver (`send.rs`) so the frontend
//! (`useZmodemTransfers.ts`) handles both directions identically.

/// A fresh id for one transfer-manager row (`TransferTask.id` on the
/// frontend). Shared by the receive and send drivers so both mint ids the
/// same way.
pub fn new_transfer_id() -> String {
    format!("zmodem-{}", uuid::Uuid::new_v4())
}

#[cfg(test)]
#[path = "events_legacy_tests.rs"]
mod legacy_contract_tests;
