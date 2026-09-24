use crate::config::ensure_config_dir;
use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LegacySshPasswordRecord {
    pub profile_name: String,
    pub password: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LegacySshPasswordStore {
    #[serde(default)]
    pub profiles: Vec<LegacySshPasswordRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnownHostRecord {
    #[serde(default)]
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    pub trusted_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct KnownHostStore {
    #[serde(default)]
    pub entries: Vec<KnownHostRecord>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SshHostKeyPromptPayload {
    pub request_id: String,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    pub reason: String,
    pub known_fingerprint: Option<String>,
}

pub fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn legacy_password_store_path() -> Result<PathBuf, String> {
    Ok(ensure_config_dir()?.join("ssh_profiles.json"))
}

pub fn load_legacy_password_store() -> Result<LegacySshPasswordStore, String> {
    let path = legacy_password_store_path()?;
    if !path.exists() {
        return Ok(LegacySshPasswordStore::default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read SSH password store: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse SSH password store: {}", e))
}

pub fn remove_legacy_password_store() -> Result<(), String> {
    let path = legacy_password_store_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Failed to remove SSH password store: {}", e))?;
    }
    Ok(())
}

pub(crate) fn list_known_hosts(connection: &Connection) -> Result<KnownHostStore, String> {
    let mut statement = connection
        .prepare(
            "SELECT profile_id, profile_name, host, port, algorithm, fingerprint, trusted_at \
             FROM known_hosts ORDER BY id",
        )
        .map_err(sql_error("Failed to read known hosts"))?;
    let entries = statement
        .query_map([], |row| {
            Ok(KnownHostRecord {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                algorithm: row.get(4)?,
                fingerprint: row.get(5)?,
                trusted_at: row.get(6)?,
            })
        })
        .and_then(Iterator::collect)
        .map_err(sql_error("Failed to read known hosts"))?;
    Ok(KnownHostStore { entries })
}

fn insert_known_host(connection: &Connection, entry: &KnownHostRecord) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO known_hosts \
             (profile_id, profile_name, host, port, algorithm, fingerprint, trusted_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.profile_id,
                entry.profile_name,
                entry.host,
                entry.port,
                entry.algorithm,
                entry.fingerprint,
                entry.trusted_at
            ],
        )
        .map_err(sql_error("Failed to save known host"))?;
    Ok(())
}

/// Adds an entry from a backup, replacing the entry it stands for: the same
/// profile id and endpoint, or for entries without a profile id, the same
/// profile name and endpoint among entries without one. Unlike
/// [`save_known_host`], an entry without an id never overwrites one with an
/// id, so an old name-only record cannot clobber the current key of the
/// profile that has since been given that name.
pub(crate) fn merge_known_host(
    connection: &Connection,
    entry: &KnownHostRecord,
) -> Result<(), String> {
    let existing_id = connection
        .query_row(
            "SELECT id FROM known_hosts WHERE host = ?1 AND port = ?2 AND \
             ((?3 IS NOT NULL AND profile_id = ?3) OR \
              (?3 IS NULL AND profile_id IS NULL AND profile_name = ?4)) \
             ORDER BY id LIMIT 1",
            params![entry.host, entry.port, entry.profile_id, entry.profile_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sql_error("Failed to read known hosts"))?;
    match existing_id {
        Some(id) => {
            connection
                .execute(
                    "UPDATE known_hosts SET profile_name = ?1, algorithm = ?2, fingerprint = ?3, \
                     trusted_at = ?4 WHERE id = ?5",
                    params![
                        entry.profile_name,
                        entry.algorithm,
                        entry.fingerprint,
                        entry.trusted_at,
                        id
                    ],
                )
                .map_err(sql_error("Failed to save known host"))?;
            Ok(())
        }
        None => insert_known_host(connection, entry),
    }
}

pub(crate) fn replace_known_hosts(
    connection: &Connection,
    store: &KnownHostStore,
) -> Result<(), String> {
    connection
        .execute("DELETE FROM known_hosts", [])
        .map_err(sql_error("Failed to clear known hosts"))?;
    for entry in &store.entries {
        insert_known_host(connection, entry)?;
    }
    Ok(())
}

pub fn load_known_host(
    profile_name: &str,
    profile_id: Option<&str>,
    host: &str,
    port: u16,
) -> Result<Option<KnownHostRecord>, String> {
    let store = crate::db::read(list_known_hosts)?;
    let profile_id = profile_id.map(str::trim).filter(|value| !value.is_empty());
    let host = host.trim();

    if let Some(profile_id) = profile_id {
        if let Some(entry) =
            find_known_host_entry(&store, profile_name, Some(profile_id), host, port)
        {
            return Ok(Some(entry));
        }
    }

    let mut entry = find_known_host_entry(&store, profile_name, None, host, port);

    if let (Some(profile_id), Some(record)) = (profile_id, entry.as_mut()) {
        if record.profile_id.as_deref() != Some(profile_id) {
            record.profile_id = Some(profile_id.to_string());
            let _ = save_known_host_entry(record.clone());
        }
    }

    Ok(entry)
}

fn find_known_host_entry(
    store: &KnownHostStore,
    profile_name: &str,
    profile_id: Option<&str>,
    host: &str,
    port: u16,
) -> Option<KnownHostRecord> {
    store
        .entries
        .iter()
        .find(|entry| {
            profile_id
                .map(|value| entry.profile_id.as_deref() == Some(value))
                .unwrap_or_else(|| entry.profile_name == profile_name)
                && entry.host == host
                && entry.port == port
        })
        .cloned()
}

pub fn save_known_host_entry(entry: KnownHostRecord) -> Result<(), String> {
    crate::db::write(|transaction| save_known_host(transaction, &entry))
}

/// Replaces the entry for the same profile id and endpoint, else the entry for
/// the same profile name and endpoint, else adds one.
fn save_known_host(connection: &Connection, entry: &KnownHostRecord) -> Result<(), String> {
    let existing_id = connection
        .query_row(
            "SELECT id FROM known_hosts \
             WHERE host = ?1 AND port = ?2 AND (profile_id = ?3 OR profile_name = ?4) \
             ORDER BY CASE WHEN profile_id = ?3 THEN 0 ELSE 1 END, id \
             LIMIT 1",
            params![entry.host, entry.port, entry.profile_id, entry.profile_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sql_error("Failed to read known hosts"))?;
    match existing_id {
        Some(id) => {
            connection
                .execute(
                    "UPDATE known_hosts SET profile_id = ?1, profile_name = ?2, algorithm = ?3, \
                     fingerprint = ?4, trusted_at = ?5 WHERE id = ?6",
                    params![
                        entry.profile_id,
                        entry.profile_name,
                        entry.algorithm,
                        entry.fingerprint,
                        entry.trusted_at,
                        id
                    ],
                )
                .map_err(sql_error("Failed to save known host"))?;
            Ok(())
        }
        None => insert_known_host(connection, entry),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        find_known_host_entry, list_known_hosts, save_known_host, KnownHostRecord, KnownHostStore,
    };
    use crate::db::Database;

    #[test]
    fn saving_a_known_host_prefers_the_profile_id_match() {
        let database = Database::open_in_memory().expect("open database");
        database
            .write(|connection| {
                save_known_host(
                    connection,
                    &record(None, "prod", "example.com", 22, "by-name"),
                )?;
                save_known_host(
                    connection,
                    &record(Some("p1"), "other", "example.com", 22, "by-id"),
                )?;
                // Matches the id entry, not the older name entry for "prod".
                save_known_host(
                    connection,
                    &record(Some("p1"), "prod", "example.com", 22, "rotated"),
                )?;
                // No id: matches by name.
                save_known_host(
                    connection,
                    &record(None, "prod", "example.com", 22, "renamed"),
                )?;
                // Different port: a new entry.
                save_known_host(
                    connection,
                    &record(None, "prod", "example.com", 2222, "alt"),
                )?;

                let fingerprints = list_known_hosts(connection)?
                    .entries
                    .into_iter()
                    .map(|entry| entry.fingerprint)
                    .collect::<Vec<_>>();
                assert_eq!(fingerprints, ["renamed", "rotated", "alt"]);
                Ok(())
            })
            .expect("save known hosts");
    }

    fn record(
        profile_id: Option<&str>,
        profile_name: &str,
        host: &str,
        port: u16,
        fingerprint: &str,
    ) -> KnownHostRecord {
        KnownHostRecord {
            profile_id: profile_id.map(str::to_string),
            profile_name: profile_name.to_string(),
            host: host.to_string(),
            port,
            algorithm: "ssh-ed25519".to_string(),
            fingerprint: fingerprint.to_string(),
            trusted_at: 1,
        }
    }

    #[test]
    fn known_host_lookup_includes_host_and_port_when_profile_id_matches() {
        let store = KnownHostStore {
            entries: vec![
                record(Some("profile-1"), "prod", "example.com", 22, "first"),
                record(Some("profile-1"), "prod", "example.net", 2222, "second"),
            ],
        };

        let entry = find_known_host_entry(&store, "prod", Some("profile-1"), "example.net", 2222)
            .expect("known host should match host and port");

        assert_eq!(entry.fingerprint, "second");
    }

    #[test]
    fn known_host_lookup_does_not_match_same_profile_different_host() {
        let store = KnownHostStore {
            entries: vec![record(
                Some("profile-1"),
                "prod",
                "example.com",
                22,
                "fingerprint",
            )],
        };

        assert!(
            find_known_host_entry(&store, "prod", Some("profile-1"), "example.net", 22).is_none()
        );
    }
}
