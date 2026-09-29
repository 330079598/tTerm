//! The merge base: what this device and the server agreed on at the last
//! sync. It holds saved passwords, so it is encrypted with the data key.

use super::document::{Collections, KdfSpec};
use crate::db::sql_error;
use crate::ssh::secret_store::{decrypt_secret, encrypt_secret, DataKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Names the base in the ciphertext's associated data.
const BASE_KEY: &str = "sync:base";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBase {
    collections: Collections,
    /// The remote file's key settings, so the next upload keeps its salt.
    kdf: Option<KdfSpec>,
    /// The revision history of the remote file the base equals.
    #[serde(default)]
    history: Vec<String>,
}

pub(crate) struct SyncBase {
    pub collections: Collections,
    pub kdf: Option<KdfSpec>,
    pub history: Vec<String>,
    pub etag: Option<String>,
}

impl SyncBase {
    /// The revision of the remote file the base equals.
    pub fn revision(&self) -> Option<&str> {
        self.history.last().map(String::as_str)
    }
}

/// The base for `target`, or `None` before the first sync with it. A base
/// that no longer decrypts (the data key was reset) counts as none: the next
/// sync then merges like a first one, which never deletes anything.
pub(crate) fn load(
    connection: &Connection,
    data_key: &DataKey,
    target: &str,
) -> Result<Option<SyncBase>, String> {
    let row = connection
        .query_row(
            "SELECT target, base_nonce, base_ciphertext, remote_etag FROM sync_state WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .map_err(sql_error("Failed to read sync state"))?;
    let Some((stored_target, nonce, ciphertext, etag)) = row else {
        return Ok(None);
    };
    if stored_target != target {
        return Ok(None);
    }
    let Ok(json) = decrypt_secret(data_key, BASE_KEY, &nonce, &ciphertext) else {
        return Ok(None);
    };
    let stored: StoredBase = serde_json::from_str(&json)
        .map_err(|error| format!("Failed to parse sync state: {error}"))?;
    Ok(Some(SyncBase {
        collections: stored.collections,
        kdf: stored.kdf,
        history: stored.history,
        etag,
    }))
}

pub(crate) fn save(
    connection: &Connection,
    data_key: &DataKey,
    target: &str,
    base: &SyncBase,
    synced_at: i64,
) -> Result<(), String> {
    let json = zeroize::Zeroizing::new(
        serde_json::to_string(&StoredBase {
            collections: base.collections.clone(),
            kdf: base.kdf.clone(),
            history: base.history.clone(),
        })
        .map_err(|error| format!("Failed to serialize sync state: {error}"))?,
    );
    let sealed = encrypt_secret(data_key, BASE_KEY, &json)?;
    connection
        .execute(
            "INSERT INTO sync_state (id, target, base_nonce, base_ciphertext, remote_etag, synced_at) \
             VALUES (1, ?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET target = excluded.target, \
             base_nonce = excluded.base_nonce, base_ciphertext = excluded.base_ciphertext, \
             remote_etag = excluded.remote_etag, synced_at = excluded.synced_at",
            params![target, sealed.nonce, sealed.ciphertext, base.etag, synced_at],
        )
        .map_err(sql_error("Failed to save sync state"))?;
    Ok(())
}

/// When this device last synced with `target`.
pub(crate) fn synced_at(
    connection: &Connection,
    target: Option<&str>,
) -> Result<Option<i64>, String> {
    connection
        .query_row(
            "SELECT target, synced_at FROM sync_state WHERE id = 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(sql_error("Failed to read sync state"))
        .map(|row| {
            row.filter(|(stored, _)| target.is_none_or(|target| target == stored))
                .map(|(_, at)| at)
        })
}

/// Forgets the base, so the next sync merges like a first one.
pub(crate) fn clear(connection: &Connection) -> Result<(), String> {
    connection
        .execute("DELETE FROM sync_state", [])
        .map_err(sql_error("Failed to reset sync state"))?;
    Ok(())
}
