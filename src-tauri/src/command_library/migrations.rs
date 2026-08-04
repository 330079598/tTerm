use rusqlite::{params, Connection};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "command_library",
    sql: include_str!("../../migrations/0001_command_library.sql"),
}];

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
            "Command database schema version {current_version} is newer than this application supports"
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
        assert_eq!(version, 1);

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
