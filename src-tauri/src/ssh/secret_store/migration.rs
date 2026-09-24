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
mod tests {
    use super::super::legacy::tests::{temp_dir, write_legacy_vault};
    use super::super::tests::MemoryCredentials;
    use super::super::{open_with_password, open_with_system_key};
    use super::*;

    fn sources<'a>(old_mode: &'a str, vault: &'a LegacyVault) -> LegacySources<'a> {
        LegacySources {
            old_mode,
            keyring_available: true,
            vault,
            password: None,
            plaintext: Vec::new(),
            candidate_keys: Vec::new(),
        }
    }

    fn secret(database: &Database, data_key: &SecretKey, key: &str) -> Option<String> {
        database
            .read(|connection| store::get_secret(connection, data_key, key))
            .unwrap()
            .map(|value| value.to_string())
    }

    #[test]
    fn hybrid_moves_the_vault_and_keeps_its_password_for_recovery() {
        let dir = temp_dir("migrate-hybrid");
        write_legacy_vault(
            &dir,
            "vault-pass",
            &[("p1", "one"), ("p1:jump:h:22:u", "two")],
        );
        let vault = LegacyVault::load(&dir).unwrap();
        let credentials = MemoryCredentials::with(&[
            (LEGACY_VAULT_MASTER_ACCOUNT, "vault-pass"),
            ("password::p1", "stale keychain copy"),
            ("password::orphan", "left over"),
        ]);
        let database = Database::open_in_memory().unwrap();

        let migration = migrate(&database, &credentials, sources("hybrid", &vault)).unwrap();
        assert_eq!(migration.mode, SecretStorageMode::System);
        assert_eq!(migration.migrated, 2);
        let data_key = migration.data_key.unwrap();
        assert_eq!(secret(&database, &data_key, "p1").as_deref(), Some("one"));
        assert_eq!(
            secret(&database, &data_key, "p1:jump:h:22:u").as_deref(),
            Some("two")
        );
        // The vault was the source of truth; stale entries are dropped unread.
        assert_eq!(secret(&database, &data_key, "orphan"), None);
        assert_eq!(credentials.reads(), [LEGACY_VAULT_MASTER_ACCOUNT]);
        let mut to_delete = migration.cleanup.keyring_accounts.clone();
        to_delete.sort();
        assert_eq!(
            to_delete,
            [
                LEGACY_VAULT_MASTER_ACCOUNT,
                "password::orphan",
                "password::p1"
            ]
        );
        assert!(migration.cleanup.retire_vault);

        let reopened = open_with_system_key(&database, &credentials)
            .unwrap()
            .unwrap();
        assert_eq!(secret(&database, &reopened, "p1").as_deref(), Some("one"));
        let recovered = open_with_password(&database, "vault-pass")
            .unwrap()
            .unwrap();
        assert_eq!(secret(&database, &recovered, "p1").as_deref(), Some("one"));
        assert!(open_with_password(&database, "nope").is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn hybrid_without_a_saved_master_waits_for_the_vault_password() {
        let dir = temp_dir("migrate-hybrid-pending");
        write_legacy_vault(&dir, "vault-pass", &[("p1", "one")]);
        let vault = LegacyVault::load(&dir).unwrap();
        let credentials = MemoryCredentials::with(&[]);
        let database = Database::open_in_memory().unwrap();

        let error = migrate(&database, &credentials, sources("hybrid", &vault)).unwrap_err();
        assert_eq!(error, NEEDS_VAULT_PASSWORD);
        assert!(!database.read(store::migrated).unwrap());

        let wrong = LegacySources {
            password: Some("wrong"),
            ..sources("hybrid", &vault)
        };
        assert!(migrate(&database, &credentials, wrong).is_err());
        assert!(!database.read(store::migrated).unwrap());

        let typed = LegacySources {
            password: Some("vault-pass"),
            ..sources("hybrid", &vault)
        };
        let migration = migrate(&database, &credentials, typed).unwrap();
        assert_eq!(
            secret(&database, migration.data_key.as_ref().unwrap(), "p1").as_deref(),
            Some("one")
        );
        assert!(database.read(store::migrated).unwrap());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn vault_mode_becomes_master_password_mode() {
        let dir = temp_dir("migrate-vault");
        write_legacy_vault(&dir, "vault-pass", &[("p1", "one")]);
        let vault = LegacyVault::load(&dir).unwrap();
        let credentials = MemoryCredentials::with(&[]);
        let database = Database::open_in_memory().unwrap();
        let typed = LegacySources {
            password: Some("vault-pass"),
            ..sources("vault", &vault)
        };

        let migration = migrate(&database, &credentials, typed).unwrap();
        assert_eq!(migration.mode, SecretStorageMode::Password);
        assert!(credentials.get(DATA_KEY_ACCOUNT).is_none());
        assert!(!database.read(|c| store::has_wrap(c, WRAP_SYSTEM)).unwrap());
        let data_key = open_with_password(&database, "vault-pass")
            .unwrap()
            .unwrap();
        assert_eq!(secret(&database, &data_key, "p1").as_deref(), Some("one"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn system_mode_reads_each_entry_and_keeps_unreadable_ones() {
        let dir = temp_dir("migrate-system");
        let vault = LegacyVault::load(&dir).unwrap();
        let credentials = MemoryCredentials::with(&[
            ("password::p1", "one"),
            ("password::legacy name", "two"),
            ("password::denied", "three"),
            ("unrelated", "x"),
        ]);
        credentials.deny("password::denied");
        let database = Database::open_in_memory().unwrap();
        let with_plaintext = LegacySources {
            plaintext: vec![
                (
                    "legacy name".to_string(),
                    Zeroizing::new("plain".to_string()),
                ),
                ("plain only".to_string(), Zeroizing::new("four".to_string())),
            ],
            ..sources("system", &vault)
        };

        let migration = migrate(&database, &credentials, with_plaintext).unwrap();
        assert_eq!(migration.mode, SecretStorageMode::System);
        let data_key = migration.data_key.unwrap();
        assert_eq!(secret(&database, &data_key, "p1").as_deref(), Some("one"));
        // The credential store copy is newer than the plaintext file.
        assert_eq!(
            secret(&database, &data_key, "legacy name").as_deref(),
            Some("two")
        );
        assert_eq!(
            secret(&database, &data_key, "plain only").as_deref(),
            Some("four")
        );
        assert_eq!(secret(&database, &data_key, "denied"), None);
        assert_eq!(migration.cleanup.unreadable_accounts, ["password::denied"]);
        assert!(!migration
            .cleanup
            .keyring_accounts
            .contains(&"password::denied".to_string()));
        assert!(migration.cleanup.remove_plaintext);
        assert!(!migration.cleanup.retire_vault);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_fresh_install_starts_with_an_empty_store() {
        let dir = temp_dir("migrate-fresh");
        let vault = LegacyVault::load(&dir).unwrap();
        let credentials = MemoryCredentials::with(&[]);
        let database = Database::open_in_memory().unwrap();

        let migration = migrate(&database, &credentials, sources("hybrid", &vault)).unwrap();
        assert_eq!(migration.mode, SecretStorageMode::System);
        assert_eq!(migration.migrated, 0);
        assert!(migration.data_key.is_some());
        assert!(open_with_system_key(&database, &credentials)
            .unwrap()
            .is_some());

        // Without a credential store there is no key source until the user
        // picks a master password.
        let database = Database::open_in_memory().unwrap();
        let no_keyring = LegacySources {
            keyring_available: false,
            ..sources("hybrid", &vault)
        };
        let migration = migrate(&database, &credentials, no_keyring).unwrap();
        assert_eq!(migration.mode, SecretStorageMode::Password);
        assert!(migration.data_key.is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
