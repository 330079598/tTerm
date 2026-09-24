//! Rows of the `secrets` and `secret_key_wraps` tables. Everything here is
//! ciphertext; encryption happens in `crypto`.

use super::crypto::{self, KdfParams, SecretKey};
use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension};
use zeroize::Zeroizing;

pub(crate) const WRAP_SYSTEM: &str = "system";
pub(crate) const WRAP_PASSWORD: &str = "password";
const MIGRATED_MARKER: &str = "secrets_migrated_at";

pub(crate) struct KeyWrap {
    pub kdf: Option<KdfParams>,
    pub nonce: Vec<u8>,
    pub wrapped_key: Vec<u8>,
}

pub(crate) fn load_wrap(connection: &Connection, kind: &str) -> Result<Option<KeyWrap>, String> {
    let row = connection
        .query_row(
            "SELECT kdf, nonce, wrapped_key FROM secret_key_wraps WHERE kind = ?1",
            [kind],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error("Failed to read the saved password key"))?;
    let Some((kdf, nonce, wrapped_key)) = row else {
        return Ok(None);
    };
    let kdf = kdf
        .map(|value| serde_json::from_str::<KdfParams>(&value))
        .transpose()
        .map_err(|error| format!("Invalid master password settings: {error}"))?;
    Ok(Some(KeyWrap {
        kdf,
        nonce,
        wrapped_key,
    }))
}

pub(crate) fn has_wrap(connection: &Connection, kind: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM secret_key_wraps WHERE kind = ?1",
            [kind],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error("Failed to read the saved password key"))
}

/// Stores the data key wrapped by `wrapping_key`, replacing any earlier wrap
/// of the same kind.
pub(crate) fn save_wrap(
    connection: &Connection,
    kind: &str,
    kdf: Option<&KdfParams>,
    wrapping_key: &SecretKey,
    data_key: &SecretKey,
) -> Result<(), String> {
    let sealed = crypto::wrap_data_key(wrapping_key, kind, data_key)?;
    let kdf = kdf
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("Failed to serialize master password settings: {error}"))?;
    connection
        .execute(
            "INSERT INTO secret_key_wraps (kind, kdf, nonce, wrapped_key, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(kind) DO UPDATE SET kdf = excluded.kdf, nonce = excluded.nonce, \
             wrapped_key = excluded.wrapped_key, created_at = excluded.created_at",
            params![
                kind,
                kdf,
                sealed.nonce,
                sealed.ciphertext,
                crate::ssh::now_unix_ms()
            ],
        )
        .map_err(sql_error("Failed to save the saved password key"))?;
    Ok(())
}

pub(crate) fn delete_wrap(connection: &Connection, kind: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM secret_key_wraps WHERE kind = ?1", [kind])
        .map_err(sql_error("Failed to delete the saved password key"))?;
    Ok(())
}

pub(crate) fn get_secret(
    connection: &Connection,
    data_key: &SecretKey,
    key: &str,
) -> Result<Option<Zeroizing<String>>, String> {
    let row = connection
        .query_row(
            "SELECT nonce, ciphertext FROM secrets WHERE key = ?1",
            [key],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()
        .map_err(sql_error("Failed to read saved password"))?;
    row.map(|(nonce, ciphertext)| crypto::decrypt_secret(data_key, key, &nonce, &ciphertext))
        .transpose()
}

/// Saves a secret. A key that names an existing profile (its id, a jump
/// host key starting with it, or its `:sudo` key) is linked to the profile so it is deleted with it.
pub(crate) fn put_secret(
    connection: &Connection,
    data_key: &SecretKey,
    key: &str,
    plaintext: &str,
) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Secret key is required".to_string());
    }
    let sealed = crypto::encrypt_secret(data_key, key, plaintext)?;
    connection
        .execute(
            "INSERT INTO secrets (key, profile_id, nonce, ciphertext, updated_at) \
             VALUES (?1, (SELECT id FROM profiles WHERE id IN (?1, \
                 CASE WHEN instr(?1, ':jump:') > 0 THEN substr(?1, 1, instr(?1, ':jump:') - 1) END, \
                 CASE WHEN substr(?1, -5) = ':sudo' THEN substr(?1, 1, length(?1) - 5) END)), \
                 ?2, ?3, ?4) \
             ON CONFLICT(key) DO UPDATE SET profile_id = excluded.profile_id, \
             nonce = excluded.nonce, ciphertext = excluded.ciphertext, updated_at = excluded.updated_at",
            params![key, sealed.nonce, sealed.ciphertext, crate::ssh::now_unix_ms()],
        )
        .map_err(sql_error("Failed to save password"))?;
    Ok(())
}

pub(crate) fn delete_secret(connection: &Connection, key: &str) -> Result<bool, String> {
    connection
        .execute("DELETE FROM secrets WHERE key = ?1", [key])
        .map(|deleted| deleted > 0)
        .map_err(sql_error("Failed to delete saved password"))
}

pub(crate) fn secret_keys(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT key FROM secrets ORDER BY key")
        .map_err(sql_error("Failed to list saved passwords"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error("Failed to list saved passwords"))?;
    rows.collect::<Result<_, _>>()
        .map_err(sql_error("Failed to list saved passwords"))
}

/// Ciphertext rows as stored, for undoing a failed backup import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SecretRow {
    pub key: String,
    pub profile_id: Option<String>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub updated_at: i64,
}

pub(crate) fn snapshot_secrets(connection: &Connection) -> Result<Vec<SecretRow>, String> {
    let mut statement = connection
        .prepare("SELECT key, profile_id, nonce, ciphertext, updated_at FROM secrets ORDER BY key")
        .map_err(sql_error("Failed to read saved passwords"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(SecretRow {
                key: row.get(0)?,
                profile_id: row.get(1)?,
                nonce: row.get(2)?,
                ciphertext: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(sql_error("Failed to read saved passwords"))?;
    rows.collect::<Result<_, _>>()
        .map_err(sql_error("Failed to read saved passwords"))
}

/// Puts the rows back exactly. The profiles they point at must exist.
pub(crate) fn restore_secrets(connection: &Connection, rows: &[SecretRow]) -> Result<(), String> {
    connection
        .execute("DELETE FROM secrets", [])
        .map_err(sql_error("Failed to clear saved passwords"))?;
    let mut statement = connection
        .prepare(
            "INSERT INTO secrets (key, profile_id, nonce, ciphertext, updated_at) \
             VALUES (?1, (SELECT id FROM profiles WHERE id = ?2), ?3, ?4, ?5)",
        )
        .map_err(sql_error("Failed to restore saved passwords"))?;
    for row in rows {
        statement
            .execute(params![
                row.key,
                row.profile_id,
                row.nonce,
                row.ciphertext,
                row.updated_at
            ])
            .map_err(sql_error("Failed to restore saved passwords"))?;
    }
    Ok(())
}

pub(crate) fn migrated(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM app_meta WHERE key = ?1",
            [MIGRATED_MARKER],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error("Failed to read the database state"))
}

pub(crate) fn mark_migrated(connection: &Connection) -> Result<(), String> {
    connection
        .execute(
            "INSERT OR REPLACE INTO app_meta (key, value) \
             VALUES (?1, CAST(unixepoch('subsec') * 1000 AS INTEGER))",
            [MIGRATED_MARKER],
        )
        .map_err(sql_error("Failed to record the password migration"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn insert_profile(connection: &Connection, id: &str) {
        connection
            .execute(
                "INSERT INTO profiles (id, position, data) VALUES (?1, 0, json_object('name', ?1))",
                [id],
            )
            .unwrap();
    }

    fn profile_of(connection: &Connection, key: &str) -> Option<String> {
        connection
            .query_row(
                "SELECT profile_id FROM secrets WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn secrets_round_trip_and_follow_their_profile() {
        let database = Database::open_in_memory().unwrap();
        let data_key = SecretKey::generate();
        database
            .write(|connection| {
                insert_profile(connection, "p1");
                put_secret(connection, &data_key, "p1", "target")?;
                put_secret(connection, &data_key, "p1:jump:bastion:22:root", "jump")?;
                put_secret(connection, &data_key, "p1:sudo", "sudo")?;
                put_secret(connection, &data_key, "legacy-name", "old")?;
                assert_eq!(profile_of(connection, "p1:sudo").as_deref(), Some("p1"));
                assert_eq!(profile_of(connection, "p1").as_deref(), Some("p1"));
                assert_eq!(
                    profile_of(connection, "p1:jump:bastion:22:root").as_deref(),
                    Some("p1")
                );
                assert_eq!(profile_of(connection, "legacy-name"), None);
                assert_eq!(
                    get_secret(connection, &data_key, "p1")?.unwrap().as_str(),
                    "target"
                );

                connection
                    .execute("DELETE FROM profiles WHERE id = 'p1'", [])
                    .unwrap();
                assert_eq!(secret_keys(connection)?, ["legacy-name"]);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn a_secret_saved_before_its_profile_is_linked_later() {
        let database = Database::open_in_memory().unwrap();
        let data_key = SecretKey::generate();
        database
            .write(|connection| {
                put_secret(connection, &data_key, "p2", "pw")?;
                put_secret(connection, &data_key, "p2:jump:h:22:u", "pw")?;
                put_secret(connection, &data_key, "p2:sudo", "pw")?;
                put_secret(connection, &data_key, "p20", "other profile")?;
                put_secret(connection, &data_key, "p20:sudo", "other profile")?;
                insert_profile(connection, "p2");
                assert_eq!(profile_of(connection, "p2:sudo").as_deref(), Some("p2"));
                assert_eq!(profile_of(connection, "p20:sudo"), None);
                assert_eq!(profile_of(connection, "p2").as_deref(), Some("p2"));
                assert_eq!(
                    profile_of(connection, "p2:jump:h:22:u").as_deref(),
                    Some("p2")
                );
                assert_eq!(profile_of(connection, "p20"), None);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn wraps_replace_by_kind() {
        let database = Database::open_in_memory().unwrap();
        let data_key = SecretKey::generate();
        let kek = SecretKey::generate();
        database
            .write(|connection| {
                assert!(!has_wrap(connection, WRAP_SYSTEM)?);
                save_wrap(
                    connection,
                    WRAP_SYSTEM,
                    None,
                    &SecretKey::generate(),
                    &data_key,
                )?;
                save_wrap(connection, WRAP_SYSTEM, None, &kek, &data_key)?;
                let wrap = load_wrap(connection, WRAP_SYSTEM)?.unwrap();
                let unwrapped =
                    crypto::unwrap_data_key(&kek, WRAP_SYSTEM, &wrap.nonce, &wrap.wrapped_key)
                        .unwrap();
                assert_eq!(unwrapped.to_base64(), data_key.to_base64());
                delete_wrap(connection, WRAP_SYSTEM)?;
                assert!(load_wrap(connection, WRAP_SYSTEM)?.is_none());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn snapshot_restores_rows_exactly() {
        let database = Database::open_in_memory().unwrap();
        let data_key = SecretKey::generate();
        database
            .write(|connection| {
                insert_profile(connection, "p1");
                put_secret(connection, &data_key, "p1", "one")?;
                put_secret(connection, &data_key, "name", "two")?;
                let before = snapshot_secrets(connection)?;
                put_secret(connection, &data_key, "p1", "changed")?;
                put_secret(connection, &data_key, "new", "three")?;
                restore_secrets(connection, &before)?;
                assert_eq!(snapshot_secrets(connection)?, before);
                Ok(())
            })
            .unwrap();
    }
}
