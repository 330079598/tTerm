use super::types::TunnelRule;
use crate::db::sql_error;
use rusqlite::{params, Connection};

/// Tunnels in display order.
pub(crate) fn load_tunnels() -> Result<Vec<TunnelRule>, String> {
    crate::db::read(list_tunnel_rules)
}

pub(crate) fn list_tunnel_rules(connection: &Connection) -> Result<Vec<TunnelRule>, String> {
    let mut statement = connection
        .prepare("SELECT data FROM tunnels ORDER BY position, rowid")
        .map_err(sql_error("Failed to read tunnels"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error("Failed to read tunnels"))?;
    rows.map(|data| {
        serde_json::from_str(&data.map_err(sql_error("Failed to read tunnels"))?)
            .map_err(|e| format!("Failed to parse tunnel: {e}"))
    })
    .collect()
}

/// Updates the rule in place, or appends it after the last rule.
pub(crate) fn upsert_tunnel(connection: &Connection, tunnel: &TunnelRule) -> Result<(), String> {
    let data =
        serde_json::to_string(tunnel).map_err(|e| format!("Failed to serialize tunnel: {e}"))?;
    connection
        .execute(
            "INSERT INTO tunnels (id, position, data) \
             VALUES (?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM tunnels), ?2) \
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![tunnel.id, data],
        )
        .map_err(sql_error("Failed to save tunnel"))?;
    Ok(())
}

pub(crate) fn delete_tunnel(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM tunnels WHERE id = ?1", [id])
        .map_err(sql_error("Failed to delete tunnel"))?;
    Ok(())
}

/// Replaces every rule, keeping the order given.
pub(crate) fn replace_tunnels(
    connection: &Connection,
    tunnels: &[TunnelRule],
) -> Result<(), String> {
    connection
        .execute("DELETE FROM tunnels", [])
        .map_err(sql_error("Failed to clear tunnels"))?;
    for tunnel in tunnels {
        upsert_tunnel(connection, tunnel)?;
    }
    Ok(())
}
