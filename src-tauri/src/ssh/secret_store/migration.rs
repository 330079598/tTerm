//! One-time move of saved passwords from the OS credential store and the old
//! app vault file into the database.
//!
//! Which old store is read follows the old mode: `hybrid` and `vault` kept
//! passwords in the vault file (per-password credential store entries were
//! left over and ignored), `system` and `auto` kept them in the credential
//! store. The old entries are deleted and the vault files renamed only after
//! the database transaction commits.

use super::crypto::{KdfParams, SecretKey};
use super::keyring_backend::{
    DATA_KEY_ACCOUNT, LEGACY_PASSWORD_PREFIX, LEGACY_VAULT_MASTER_ACCOUNT,
};
use super::legacy::LegacyVault;
use super::store::{self, WRAP_PASSWORD, WRAP_SYSTEM};
use super::types::SecretStorageMode;
use super::CredentialStore;
use crate::db::{sql_error, Database};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

pub(crate) const NEEDS_VAULT_PASSWORD: &str =
    "Enter the app vault password to move saved passwords into the database.";

pub(crate) struct LegacySources<'a> {
    /// `secret_storage_mode` from config.json, before the migration.
    pub old_mode: &'a str,
    pub keyring_available: bool,
    pub vault: &'a LegacyVault,
    /// The vault password, when the user typed it.
    pub password: Option<&'a str>,
    /// Plaintext passwords from the oldest format (`ssh_profiles.json`).
    pub plaintext: Vec<(String, Zeroizing<String>)>,
    /// Keys to look for when the credential store cannot be listed.
    pub candidate_keys: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct Migration {
    pub mode: SecretStorageMode,
    /// Set when saved passwords are unlocked afterwards.
    pub data_key: Option<SecretKey>,
    pub migrated: usize,
    pub cleanup: Cleanup,
}

#[derive(Debug, Default)]
pub(crate) struct Cleanup {
    /// Credential store entries that are no longer needed.
    pub keyring_accounts: Vec<String>,
    /// Entries that could not be read; they are left in place.
    pub unreadable_accounts: Vec<String>,
    pub retire_vault: bool,
    pub remove_plaintext: bool,
}

pub(crate) fn migrate(
    database: &Database,
    credentials: &dyn CredentialStore,
    sources: LegacySources<'_>,
) -> Result<Migration, String> {
    let old_mode = sources.old_mode;
    let keyring = sources.keyring_available;
    let mode = match old_mode {
        "memory" => SecretStorageMode::Memory,
        "vault" => SecretStorageMode::Password,
        _ if keyring => SecretStorageMode::System,
        _ => SecretStorageMode::Password,
    };
    if mode == SecretStorageMode::Memory {
        // Nothing was being saved; leave any old entries alone.
        database.write(|transaction| store::mark_migrated(transaction))?;
        return Ok(Migration {
            mode,
            data_key: None,
            migrated: 0,
            cleanup: Cleanup::default(),
        });
    }

    let uses_vault = match old_mode {
        "vault" | "hybrid" => true,
        _ => !keyring,
    };
    let legacy_accounts: Vec<String> = if keyring {
        credentials
            .list_accounts()
            .unwrap_or_else(|| {
                sources
                    .candidate_keys
                    .iter()
                    .map(|key| format!("{LEGACY_PASSWORD_PREFIX}{key}"))
                    .collect()
            })
            .into_iter()
            .filter(|account| account.starts_with(LEGACY_PASSWORD_PREFIX))
            .collect()
    } else {
        Vec::new()
    };

    let mut collected = BTreeMap::<String, Zeroizing<String>>::new();
    let mut master_password: Option<Zeroizing<String>> = None;
    let mut cleanup = Cleanup::default();

    if uses_vault {
        if sources.vault.has_passwords() {
            let password = match sources.password {
                Some(password) => Zeroizing::new(password.to_string()),
                None if old_mode == "hybrid" && keyring => credentials
                    .read(LEGACY_VAULT_MASTER_ACCOUNT)?
                    .ok_or_else(|| NEEDS_VAULT_PASSWORD.to_string())?,
                None => return Err(NEEDS_VAULT_PASSWORD.to_string()),
            };
            collected.extend(sources.vault.decrypt_all(&password)?);
            master_password = Some(password);
        } else if let Some(password) = sources.password {
            master_password = Some(Zeroizing::new(password.to_string()));
        }
        cleanup.retire_vault = true;
        cleanup.keyring_accounts = legacy_accounts;
    } else {
        for account in legacy_accounts {
            match credentials.read(&account) {
                Ok(Some(value)) => {
                    let key = account[LEGACY_PASSWORD_PREFIX.len()..].to_string();
                    collected.entry(key).or_insert(value);
                    cleanup.keyring_accounts.push(account);
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("Skipping unreadable credential '{account}': {error}");
                    cleanup.unreadable_accounts.push(account);
                }
            }
        }
    }
    if keyring {
        cleanup
            .keyring_accounts
            .push(LEGACY_VAULT_MASTER_ACCOUNT.to_string());
    }

    // Without a key source nothing written now could be read back later.
    let persistable = mode == SecretStorageMode::System || master_password.is_some();
    if persistable && !sources.plaintext.is_empty() {
        for (key, value) in sources.plaintext {
            if !value.is_empty() {
                collected.entry(key).or_insert(value);
            }
        }
        cleanup.remove_plaintext = true;
    }

    let data_key = persistable.then(SecretKey::generate);
    // Argon2 runs before the transaction so the database is not held for it.
    let password_wrap = master_password
        .as_deref()
        .map(|password| {
            let kdf = KdfParams::generate();
            kdf.derive(password).map(|key| (kdf, key))
        })
        .transpose()?;
    let system_key = if mode == SecretStorageMode::System {
        let key = SecretKey::generate();
        credentials.write(DATA_KEY_ACCOUNT, &key.to_base64())?;
        Some(key)
    } else {
        None
    };

    let migrated = collected.len();
    database.write(|transaction| {
        transaction
            .execute_batch("DELETE FROM secrets; DELETE FROM secret_key_wraps;")
            .map_err(sql_error("Failed to reset saved passwords"))?;
        if let Some(data_key) = data_key.as_ref() {
            if let Some(system_key) = system_key.as_ref() {
                store::save_wrap(transaction, WRAP_SYSTEM, None, system_key, data_key)?;
            }
            if let Some((kdf, password_key)) = password_wrap.as_ref() {
                store::save_wrap(
                    transaction,
                    WRAP_PASSWORD,
                    Some(kdf),
                    password_key,
                    data_key,
                )?;
            }
            for (key, value) in &collected {
                store::put_secret(transaction, data_key, key, value)?;
            }
        }
        store::mark_migrated(transaction)
    })?;

    Ok(Migration {
        mode,
        data_key,
        migrated,
        cleanup,
    })
}

#[cfg(test)]
mod tests;
