//! The `serial_send_history` table.

use super::{SerialSend, SerialSendMode, HISTORY_LIMIT};
use crate::db::sql_error;
use rusqlite::{params, Connection, Row};
use sha2::{Digest, Sha256};

/// The id of a send: the same on every device, so syncing two devices that
/// sent the same thing keeps one record.
pub(crate) fn send_id(connection_key: &str, mode: SerialSendMode, text: &str) -> String {
    let digest = Sha256::digest(format!("{connection_key}\0{}\0{text}", mode.as_str()));
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<SerialSend> {
    let mode: String = row.get(2)?;
    Ok(SerialSend {
        id: row.get(0)?,
        connection_key: row.get(1)?,
        // The table's CHECK keeps other modes out.
        mode: SerialSendMode::parse(&mode).unwrap_or(SerialSendMode::Text),
        text: row.get(3)?,
        sent_at: row.get(4)?,
    })
}

const COLUMNS: &str = "id, connection_key, mode, text, sent_at";

/// A connection's sends, newest first.
pub(crate) fn list(
    connection: &Connection,
    connection_key: &str,
) -> Result<Vec<SerialSend>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {COLUMNS} FROM serial_send_history WHERE connection_key = ?1 \
             ORDER BY sent_at DESC, id LIMIT ?2"
        ))
        .map_err(sql_error("Failed to read the serial send history"))?;
    statement
        .query_map(params![connection_key, HISTORY_LIMIT as i64], from_row)
        .and_then(Iterator::collect)
        .map_err(sql_error("Failed to read the serial send history"))
}

/// Every connection's sends.
pub(crate) fn list_all(connection: &Connection) -> Result<Vec<SerialSend>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {COLUMNS} FROM serial_send_history ORDER BY connection_key, sent_at DESC, id"
        ))
        .map_err(sql_error("Failed to read the serial send history"))?;
    statement
        .query_map([], from_row)
        .and_then(Iterator::collect)
        .map_err(sql_error("Failed to read the serial send history"))
}

/// Saves `send` as it is, over a record with its id.
pub(crate) fn put(connection: &Connection, send: &SerialSend) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO serial_send_history (id, connection_key, mode, text, sent_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET connection_key = excluded.connection_key, \
             mode = excluded.mode, text = excluded.text, sent_at = excluded.sent_at",
            params![
                send.id,
                send.connection_key,
                send.mode.as_str(),
                send.text,
                send.sent_at
            ],
        )
        .map_err(sql_error("Failed to save the serial send history"))?;
    Ok(())
}

/// Records a send at `sent_at` and drops what falls past the limit.
pub(crate) fn record(
    connection: &Connection,
    connection_key: &str,
    mode: SerialSendMode,
    text: &str,
    sent_at: i64,
) -> Result<(), String> {
    put(
        connection,
        &SerialSend {
            id: send_id(connection_key, mode, text),
            connection_key: connection_key.to_string(),
            mode,
            text: text.to_string(),
            sent_at,
        },
    )?;
    trim(connection)
}

pub(crate) fn delete(connection: &Connection, ids: &[String]) -> Result<(), String> {
    let mut statement = connection
        .prepare("DELETE FROM serial_send_history WHERE id = ?1")
        .map_err(sql_error("Failed to delete from the serial send history"))?;
    for id in ids {
        statement
            .execute([id])
            .map_err(sql_error("Failed to delete from the serial send history"))?;
    }
    Ok(())
}

pub(crate) fn clear(connection: &Connection, connection_key: &str) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM serial_send_history WHERE connection_key = ?1",
            [connection_key],
        )
        .map_err(sql_error("Failed to clear the serial send history"))?;
    Ok(())
}

/// Replaces every connection's sends with `sends`.
pub(crate) fn replace_all(connection: &Connection, sends: &[SerialSend]) -> Result<(), String> {
    connection
        .execute("DELETE FROM serial_send_history", [])
        .map_err(sql_error("Failed to clear the serial send history"))?;
    for send in sends {
        put(connection, send)?;
    }
    trim(connection)
}

/// Keeps each connection's newest sends up to the limit. Merging two
/// devices' histories, by sync or a backup, can leave more.
pub(crate) fn trim(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM serial_send_history WHERE id IN (\
                SELECT id FROM (\
                    SELECT id, ROW_NUMBER() OVER (\
                        PARTITION BY connection_key ORDER BY sent_at DESC, id\
                    ) AS rank FROM serial_send_history\
                ) WHERE rank > ?1\
             )",
            [HISTORY_LIMIT as i64],
        )
        .map_err(sql_error("Failed to trim the serial send history"))?;
    Ok(())
}
