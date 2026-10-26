use rusqlite::{params, Connection};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "command_library",
        sql: include_str!("../../migrations/0001_command_library.sql"),
    },
    Migration {
        version: 2,
        name: "command_tag_catalog",
        sql: include_str!("../../migrations/0002_command_tag_catalog.sql"),
    },
    Migration {
        version: 3,
        name: "app_data",
        sql: include_str!("../../migrations/0003_app_data.sql"),
    },
    Migration {
        version: 4,
        name: "secrets",
        sql: include_str!("../../migrations/0004_secrets.sql"),
    },
    Migration {
        version: 5,
        name: "sudo_secrets",
        sql: include_str!("../../migrations/0005_sudo_secrets.sql"),
    },
    Migration {
        version: 6,
        name: "tunnel_traffic",
        sql: include_str!("../../migrations/0006_tunnel_traffic.sql"),
    },
    Migration {
        version: 7,
        name: "sync",
        sql: include_str!("../../migrations/0007_sync.sql"),
    },
    Migration {
        version: 8,
        name: "custom_themes",
        sql: include_str!("../../migrations/0008_custom_themes.sql"),
    },
    Migration {
        version: 9,
        name: "proxies",
        sql: include_str!("../../migrations/0009_proxies.sql"),
    },
    Migration {
        version: 10,
        name: "trusted_proxy_commands",
        sql: include_str!("../../migrations/0010_trusted_proxy_commands.sql"),
    },
    Migration {
        version: 11,
        name: "serial_send_history",
        sql: include_str!("../../migrations/0011_serial_send_history.sql"),
    },
    Migration {
        version: 12,
        name: "nested_profile_groups",
        sql: include_str!("../../migrations/0012_nested_profile_groups.sql"),
    },
];

pub(super) fn apply(connection: &mut Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                version INTEGER PRIMARY KEY,\
                name TEXT NOT NULL,\
                applied_at INTEGER NOT NULL\
            );",
        )
        .map_err(|error| format!("Failed to initialize migration metadata: {error}"))?;

    let current_version = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to read database schema version: {error}"))?;

    if current_version > MIGRATIONS.last().map_or(0, |migration| migration.version) {
        return Err(format!(
            "Database schema version {current_version} is newer than this application supports"
        ));
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        let transaction = connection.transaction().map_err(|error| {
            format!(
                "Failed to start database migration {}: {error}",
                migration.version
            )
        })?;
        transaction.execute_batch(migration.sql).map_err(|error| {
            format!(
                "Failed to apply database migration {} ({}): {error}",
                migration.version, migration.name
            )
        })?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at) \
                 VALUES (?1, ?2, unixepoch('subsec') * 1000)",
                params![migration.version, migration.name],
            )
            .map_err(|error| {
                format!(
                    "Failed to record database migration {}: {error}",
                    migration.version
                )
            })?;
        transaction.commit().map_err(|error| {
            format!(
                "Failed to commit database migration {}: {error}",
                migration.version
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent() {
        let mut connection = Connection::open_in_memory().expect("open database");

        apply(&mut connection).expect("first migration run");
        apply(&mut connection).expect("second migration run");

        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("read schema version");
        assert_eq!(version, 12);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'saved_commands'",
                [],
                |row| row.get(0),
            )
            .expect("read schema");
        assert_eq!(table_count, 1);
    }

    #[test]
    fn profile_groups_named_before_become_records_keyed_by_their_name() {
        let mut connection = Connection::open_in_memory().expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (\
                    version INTEGER PRIMARY KEY,\
                    name TEXT NOT NULL,\
                    applied_at INTEGER NOT NULL\
                );",
            )
            .expect("create migration metadata");
        for migration in MIGRATIONS.iter().filter(|migration| migration.version < 12) {
            connection
                .execute_batch(migration.sql)
                .expect("apply older migration");
            connection
                .execute(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, 0)",
                    params![migration.version, migration.name],
                )
                .expect("record older migration");
        }
        connection
            .execute_batch(
                "INSERT INTO profile_groups (name) VALUES ('empty'), ('ops');\
                 INSERT INTO profiles (id, position, data) VALUES\
                    ('a', 0, '{\"id\":\"a\",\"name\":\"A\",\"group\":\" ops \"}'),\
                    ('b', 1, '{\"id\":\"b\",\"name\":\"B\",\"group\":\"web\"}'),\
                    ('c', 2, '{\"id\":\"c\",\"name\":\"C\",\"group\":\"\"}'),\
                    ('d', 3, '{\"id\":\"d\",\"name\":\"D\"}');",
            )
            .expect("seed groups by name");

        apply(&mut connection).expect("migrate");

        let groups = connection
            .prepare("SELECT id, name, parent_id FROM profile_groups ORDER BY id")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("read groups");
        let named = |name: &str| (name.to_string(), name.to_string(), None);
        assert_eq!(groups, [named("empty"), named("ops"), named("web")]);

        let data = |id: &str| {
            connection
                .query_row("SELECT data FROM profiles WHERE id = ?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .expect("read profile")
        };
        assert_eq!(data("a"), r#"{"id":"a","name":"A","group_id":"ops"}"#);
        assert_eq!(data("b"), r#"{"id":"b","name":"B","group_id":"web"}"#);
        assert_eq!(data("c"), r#"{"id":"c","name":"C"}"#);
        assert_eq!(data("d"), r#"{"id":"d","name":"D"}"#);
    }

    #[test]
    fn newer_database_versions_are_rejected() {
        let mut connection = Connection::open_in_memory().expect("open database");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (\
                    version INTEGER PRIMARY KEY,\
                    name TEXT NOT NULL,\
                    applied_at INTEGER NOT NULL\
                );\
                INSERT INTO schema_migrations VALUES (999, 'future', 0);",
            )
            .expect("seed future migration");

        let error = apply(&mut connection).expect_err("future schema must fail");
        assert!(error.contains("newer than this application supports"));
    }
}
