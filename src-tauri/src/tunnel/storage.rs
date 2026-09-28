use super::types::{TunnelRule, TunnelTraffic};
use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::HashMap;

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
    connection
        .execute("DELETE FROM tunnel_traffic WHERE tunnel_id = ?1", [id])
        .map_err(sql_error("Failed to delete tunnel traffic"))?;
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
    // Rules that survive keep their traffic; only that of removed rules goes.
    connection
        .execute(
            "DELETE FROM tunnel_traffic WHERE tunnel_id NOT IN (SELECT id FROM tunnels)",
            [],
        )
        .map_err(sql_error("Failed to prune tunnel traffic"))?;
    Ok(())
}

fn to_sql_count(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn traffic_from_row(row: &Row<'_>) -> rusqlite::Result<TunnelTraffic> {
    let count = |index| row.get::<_, i64>(index).map(|value| value.max(0) as u64);
    Ok(TunnelTraffic {
        bytes_up: count(0)?,
        bytes_down: count(1)?,
        connections: count(2)?,
        since: Some(count(3)?),
    })
}

/// Saved traffic of every rule that has any.
pub(crate) fn list_tunnel_traffic(
    connection: &Connection,
) -> Result<HashMap<String, TunnelTraffic>, String> {
    let mut statement = connection
        .prepare("SELECT bytes_up, bytes_down, connections, since, tunnel_id FROM tunnel_traffic")
        .map_err(sql_error("Failed to read tunnel traffic"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(4)?, traffic_from_row(row)?))
        })
        .map_err(sql_error("Failed to read tunnel traffic"))?;
    rows.collect::<rusqlite::Result<_>>()
        .map_err(sql_error("Failed to read tunnel traffic"))
}

pub(crate) fn tunnel_traffic(connection: &Connection, id: &str) -> Result<TunnelTraffic, String> {
    connection
        .query_row(
            "SELECT bytes_up, bytes_down, connections, since FROM tunnel_traffic \
             WHERE tunnel_id = ?1",
            [id],
            traffic_from_row,
        )
        .optional()
        .map(Option::unwrap_or_default)
        .map_err(sql_error("Failed to read tunnel traffic"))
}

/// Adds `delta` (up, down, connections) to a rule's saved traffic, starting
/// the count at `now` if it has none. Does nothing for a rule that no longer
/// exists, so a run outliving its rule leaves no orphan behind.
pub(crate) fn add_tunnel_traffic(
    connection: &Connection,
    id: &str,
    (up, down, connections): (u64, u64, u64),
    now: u64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO tunnel_traffic \
                 (tunnel_id, bytes_up, bytes_down, connections, since, updated_at) \
             SELECT ?1, ?2, ?3, ?4, ?5, ?5 WHERE EXISTS (SELECT 1 FROM tunnels WHERE id = ?1) \
             ON CONFLICT(tunnel_id) DO UPDATE SET \
                 bytes_up = bytes_up + excluded.bytes_up, \
                 bytes_down = bytes_down + excluded.bytes_down, \
                 connections = connections + excluded.connections, \
                 updated_at = excluded.updated_at",
            params![
                id,
                to_sql_count(up),
                to_sql_count(down),
                to_sql_count(connections),
                to_sql_count(now)
            ],
        )
        .map_err(sql_error("Failed to save tunnel traffic"))?;
    Ok(())
}

/// Zeroes a rule's saved traffic and restarts the count at `now`.
pub(crate) fn reset_tunnel_traffic(
    connection: &Connection,
    id: &str,
    now: u64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO tunnel_traffic \
                 (tunnel_id, bytes_up, bytes_down, connections, since, updated_at) \
             SELECT ?1, 0, 0, 0, ?2, ?2 WHERE EXISTS (SELECT 1 FROM tunnels WHERE id = ?1) \
             ON CONFLICT(tunnel_id) DO UPDATE SET \
                 bytes_up = 0, bytes_down = 0, connections = 0, \
                 since = excluded.since, updated_at = excluded.updated_at",
            params![id, to_sql_count(now)],
        )
        .map_err(sql_error("Failed to reset tunnel traffic"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn rule(id: &str) -> TunnelRule {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "profileId": "p1", "kind": "local",
            "bindHost": "127.0.0.1", "bindPort": 8080,
            "destHost": "db", "destPort": 5432
        }))
        .unwrap()
    }

    fn database_with(ids: &[&str]) -> Database {
        let database = Database::open_in_memory().unwrap();
        let rules = ids.iter().map(|id| rule(id)).collect::<Vec<_>>();
        database
            .write(|transaction| replace_tunnels(transaction, &rules))
            .unwrap();
        database
    }

    fn traffic(database: &Database, id: &str) -> TunnelTraffic {
        database
            .read(|connection| tunnel_traffic(connection, id))
            .unwrap()
    }

    #[test]
    fn traffic_accumulates_and_keeps_its_start() {
        let database = database_with(&["a"]);
        assert_eq!(traffic(&database, "a"), TunnelTraffic::default());

        database
            .write(|t| add_tunnel_traffic(t, "a", (10, 20, 1), 1_000))
            .unwrap();
        database
            .write(|t| add_tunnel_traffic(t, "a", (5, 7, 2), 2_000))
            .unwrap();

        assert_eq!(
            traffic(&database, "a"),
            TunnelTraffic {
                bytes_up: 15,
                bytes_down: 27,
                connections: 3,
                since: Some(1_000),
            }
        );
    }

    #[test]
    fn traffic_of_a_missing_rule_is_not_saved() {
        let database = database_with(&["a"]);
        database
            .write(|t| add_tunnel_traffic(t, "gone", (10, 20, 1), 1_000))
            .unwrap();
        database
            .write(|t| reset_tunnel_traffic(t, "gone", 1_000))
            .unwrap();
        let all = database.read(list_tunnel_traffic).unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn reset_zeroes_and_restarts_the_count() {
        let database = database_with(&["a"]);
        database
            .write(|t| add_tunnel_traffic(t, "a", (10, 20, 1), 1_000))
            .unwrap();
        database
            .write(|t| reset_tunnel_traffic(t, "a", 5_000))
            .unwrap();
        assert_eq!(
            traffic(&database, "a"),
            TunnelTraffic {
                since: Some(5_000),
                ..TunnelTraffic::default()
            }
        );
    }

    #[test]
    fn traffic_follows_its_rule() {
        let database = database_with(&["a", "b", "c"]);
        for id in ["a", "b", "c"] {
            database
                .write(|t| add_tunnel_traffic(t, id, (1, 1, 1), 1_000))
                .unwrap();
        }

        database.write(|t| delete_tunnel(t, "c")).unwrap();
        // Restoring a backup rewrites every rule; survivors keep their count.
        database
            .write(|t| replace_tunnels(t, &[rule("a")]))
            .unwrap();

        let all = database.read(list_tunnel_traffic).unwrap();
        assert_eq!(all.keys().collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(all["a"].bytes_up, 1);
    }
}
