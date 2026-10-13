//! The saved proxy servers, one row each in display order.

use super::ProxyServer;
use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension};

/// Proxy servers in display order.
pub(crate) fn list_proxies(connection: &Connection) -> Result<Vec<ProxyServer>, String> {
    let mut statement = connection
        .prepare("SELECT data FROM proxies ORDER BY position, rowid")
        .map_err(sql_error("Failed to read proxies"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(sql_error("Failed to read proxies"))?;
    rows.iter().map(|data| decode(data)).collect()
}

pub(crate) fn get_proxy(connection: &Connection, id: &str) -> Result<Option<ProxyServer>, String> {
    connection
        .query_row("SELECT data FROM proxies WHERE id = ?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(sql_error("Failed to read proxy"))?
        .map(|data| decode(&data))
        .transpose()
}

/// Updates the server in place, or appends it after the last one.
pub(crate) fn upsert_proxy(connection: &Connection, server: &ProxyServer) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO proxies (id, position, data) \
             VALUES (?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM proxies), ?2) \
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![server.id, encode(server)?],
        )
        .map_err(sql_error("Failed to save proxy"))?;
    Ok(())
}

/// Writes the server at `position`, as sync places it.
pub(crate) fn put_proxy_at(
    connection: &Connection,
    server: &ProxyServer,
    position: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO proxies (id, position, data) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET position = excluded.position, data = excluded.data",
            params![server.id, position, encode(server)?],
        )
        .map_err(sql_error("Failed to save proxy"))?;
    Ok(())
}

pub(crate) fn delete_proxy(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM proxies WHERE id = ?1", [id])
        .map_err(sql_error("Failed to delete proxy"))?;
    Ok(())
}

/// Replaces every server, keeping the order given.
pub(crate) fn replace_proxies(
    connection: &Connection,
    servers: &[ProxyServer],
) -> Result<(), String> {
    connection
        .execute("DELETE FROM proxies", [])
        .map_err(sql_error("Failed to clear proxies"))?;
    for (position, server) in servers.iter().enumerate() {
        put_proxy_at(connection, server, position as i64)?;
    }
    Ok(())
}

fn encode(server: &ProxyServer) -> Result<String, String> {
    serde_json::to_string(server).map_err(|error| format!("Failed to encode proxy: {error}"))
}

fn decode(data: &str) -> Result<ProxyServer, String> {
    serde_json::from_str(data).map_err(|error| format!("Failed to parse proxy: {error}"))
}
