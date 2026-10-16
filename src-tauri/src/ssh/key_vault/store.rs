//! The key vault's keys among the saved secrets.

use super::entry::VaultKey;
use super::{reference_id, secret_key, SECRET_PREFIX};
use crate::profiles::types::SavedProfile;
use crate::ssh::secret_store::crypto::SecretKey as DataKey;
use crate::ssh::secret_store::store::{delete_secret, get_secret, put_secret, secret_keys};
use rusqlite::Connection;
use zeroize::Zeroizing;

/// The ids of the keys in the vault, readable while it is locked.
pub(super) fn ids(connection: &Connection) -> Result<Vec<String>, String> {
    Ok(secret_keys(connection)?
        .into_iter()
        .filter_map(|key| key.strip_prefix(SECRET_PREFIX).map(str::to_string))
        .collect())
}

pub(crate) fn exists(connection: &Connection, id: &str) -> Result<bool, String> {
    Ok(ids(connection)?.iter().any(|known| known == id))
}

pub(super) fn get(
    connection: &Connection,
    data_key: &DataKey,
    id: &str,
) -> Result<Option<VaultKey>, String> {
    get_secret(connection, data_key, &secret_key(id))?
        .map(|json| {
            serde_json::from_str(&json)
                .map_err(|error| format!("The key vault entry {id} is damaged: {error}"))
        })
        .transpose()
}

pub(super) fn put(
    connection: &Connection,
    data_key: &DataKey,
    id: &str,
    key: &VaultKey,
) -> Result<(), String> {
    let json = Zeroizing::new(
        serde_json::to_string(key).map_err(|error| format!("Failed to encode the key: {error}"))?,
    );
    put_secret(connection, data_key, &secret_key(id), &json)
}

pub(super) fn delete(connection: &Connection, id: &str) -> Result<bool, String> {
    delete_secret(connection, &secret_key(id))
}

/// The names of the profiles that sign in with key `id`, themselves or
/// through one of their jump hosts. A path left over from before a profile
/// switched to another method does not count.
pub(super) fn used_by(profiles: &[SavedProfile], id: &str) -> Vec<String> {
    let points_here = |path: &Option<String>| path.as_deref().and_then(reference_id) == Some(id);
    profiles
        .iter()
        .filter(|profile| {
            let own = matches!(profile.auth_method.as_deref(), Some("key" | "auto"))
                && points_here(&profile.private_key_path);
            let jumps = profile.uses_jump_host()
                && profile
                    .jump_hosts
                    .iter()
                    .any(|jump| jump.auth_method == "key" && points_here(&jump.private_key_path));
            own || jumps
        })
        .map(|profile| profile.name.clone())
        .collect()
}
