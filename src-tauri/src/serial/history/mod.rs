//! What serial send bars sent, kept per connection in the database so it
//! survives restarts and travels with sync and backups.

pub(crate) mod commands;
pub(crate) mod store;
#[cfg(test)]
mod tests;

use crate::core::blocking::run_blocking;
use serde::{Deserialize, Serialize};

/// How many sends a connection keeps; older ones are dropped.
pub(crate) const HISTORY_LIMIT: usize = 30;

/// How a send bar sent: text the way typing does, or hex bytes as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SerialSendMode {
    Text,
    Hex,
}

impl SerialSendMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Hex => "hex",
        }
    }

    fn parse(mode: &str) -> Option<Self> {
        match mode {
            "text" => Some(Self::Text),
            "hex" => Some(Self::Hex),
            _ => None,
        }
    }
}

/// One thing a connection's send bar sent, last at `sent_at`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SerialSend {
    pub id: String,
    /// `profile:<id>` for a saved profile, else `port:<port>`.
    pub connection_key: String,
    pub mode: SerialSendMode,
    pub text: String,
    /// Unix milliseconds.
    pub sent_at: i64,
}

/// Refuses keys the send bar doesn't make, so stray rows can't pile up.
fn validate_key(connection_key: &str) -> Result<(), String> {
    let name = connection_key
        .strip_prefix("profile:")
        .or_else(|| connection_key.strip_prefix("port:"));
    match name {
        Some(name) if !name.trim().is_empty() => Ok(()),
        _ => Err(format!(
            "'{connection_key}' does not name a serial connection"
        )),
    }
}

async fn list_serial_sends_service(connection_key: String) -> Result<Vec<SerialSend>, String> {
    validate_key(&connection_key)?;
    run_blocking(move || {
        tterm_core::db::read(|connection| store::list(connection, &connection_key))
    })
    .await
}

struct RecordSerialSendInput {
    connection_key: String,
    mode: SerialSendMode,
    text: String,
}

async fn record_serial_send_service(
    arguments: RecordSerialSendInput,
) -> Result<Vec<SerialSend>, String> {
    let RecordSerialSendInput {
        connection_key,
        mode,
        text,
    } = arguments;

    validate_key(&connection_key)?;
    run_blocking(move || {
        tterm_core::db::write(|transaction| {
            let sent_at = chrono::Utc::now().timestamp_millis();
            store::record(transaction, &connection_key, mode, &text, sent_at)?;
            store::list(transaction, &connection_key)
        })
    })
    .await
}

struct DeleteSerialSendInput {
    connection_key: String,
    id: String,
}

async fn delete_serial_send_service(
    arguments: DeleteSerialSendInput,
) -> Result<Vec<SerialSend>, String> {
    let DeleteSerialSendInput { connection_key, id } = arguments;

    validate_key(&connection_key)?;
    run_blocking(move || {
        tterm_core::db::write(|transaction| {
            store::delete(transaction, &[id])?;
            store::list(transaction, &connection_key)
        })
    })
    .await
}

async fn clear_serial_sends_service(connection_key: String) -> Result<(), String> {
    validate_key(&connection_key)?;
    run_blocking(move || {
        tterm_core::db::write(|transaction| store::clear(transaction, &connection_key))
    })
    .await
}
