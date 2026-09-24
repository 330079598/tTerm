//! Saved passwords, stored encrypted in the database.
//!
//! One data key encrypts every secret. It is kept wrapped by a key in the OS
//! credential store (`system` mode, unlocks at startup with one credential
//! store read) and/or by a key derived from the user's master password
//! (`password` mode, or a recovery path in `system` mode).

mod crypto;
mod keyring_backend;
mod legacy;
mod migration;
mod store;
mod types;

use crate::config::{load_config_file, save_config_file};
use crate::db::Database;
use crypto::{KdfParams, SecretKey};
use keyring_backend::DATA_KEY_ACCOUNT;
use migration::{LegacySources, NEEDS_VAULT_PASSWORD};
use std::sync::{Arc, Mutex, MutexGuard};
use store::{WRAP_PASSWORD, WRAP_SYSTEM};
use tauri::{AppHandle, Manager};
use types::{SecretStorageMode, SecretStoreRuntime};
use zeroize::Zeroizing;

pub(crate) use crypto::SecretKey as DataKey;
pub(crate) use store::{get_secret, put_secret, restore_secrets, snapshot_secrets, SecretRow};
pub use types::{
    ChangeVaultPasswordInput, SecretBackendStatus, SecretLocation, SecretStoreState,
    VaultPasswordInput,
};

const MISSING_SYSTEM_KEY: &str = "The key for saved passwords is missing from the system credential store. Enter the master password to restore it.";
const WRONG_SYSTEM_KEY: &str = "The key in the system credential store does not match saved passwords. Enter the master password to restore it.";
const WRONG_MASTER_PASSWORD: &str = "Incorrect master password.";
const LOCKED: &str = "Saved passwords are locked. Unlock them first.";

/// The OS credential store, behind a trait so the logic can be tested.
pub(crate) trait CredentialStore {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<String>>, String>;
    fn write(&self, account: &str, value: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<bool, String>;
    fn list_accounts(&self) -> Option<Vec<String>>;
}

struct OsCredentials;

impl CredentialStore for OsCredentials {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<String>>, String> {
        keyring_backend::read(account)
    }

    fn write(&self, account: &str, value: &str) -> Result<(), String> {
        keyring_backend::write(account, value)
    }

    fn delete(&self, account: &str) -> Result<bool, String> {
        keyring_backend::delete(account)
    }

    fn list_accounts(&self) -> Option<Vec<String>> {
        keyring_backend::list_accounts()
    }
}

/// Unwraps the data key with the key in the credential store. `Ok(None)`
/// means this database has no credential store wrap. The credential store is
/// read outside the database lock since it may wait on an unlock prompt.
fn open_with_system_key(
    database: &Database,
    credentials: &dyn CredentialStore,
) -> Result<Option<SecretKey>, String> {
    let Some(wrap) = database.read(|connection| store::load_wrap(connection, WRAP_SYSTEM))? else {
        return Ok(None);
    };
    let encoded = credentials
        .read(DATA_KEY_ACCOUNT)?
        .ok_or_else(|| MISSING_SYSTEM_KEY.to_string())?;
    let system_key = SecretKey::from_base64(&encoded).map_err(|_| WRONG_SYSTEM_KEY.to_string())?;
    crypto::unwrap_data_key(&system_key, WRAP_SYSTEM, &wrap.nonce, &wrap.wrapped_key)
        .map(Some)
        .ok_or_else(|| WRONG_SYSTEM_KEY.to_string())
}

/// Unwraps the data key with the master password. `Ok(None)` means no master
/// password is set. Key derivation runs outside the database lock.
fn open_with_password(database: &Database, password: &str) -> Result<Option<SecretKey>, String> {
    let Some(wrap) = database.read(|connection| store::load_wrap(connection, WRAP_PASSWORD))?
    else {
        return Ok(None);
    };
    let kdf = wrap
        .kdf
        .ok_or_else(|| "Master password settings are missing.".to_string())?;
    let password_key = kdf.derive(password)?;
    crypto::unwrap_data_key(&password_key, WRAP_PASSWORD, &wrap.nonce, &wrap.wrapped_key)
        .map(Some)
        .ok_or_else(|| WRONG_MASTER_PASSWORD.to_string())
}

/// Puts a fresh key in the credential store and wraps the data key with it.
fn create_system_wrap(
    database: &Database,
    credentials: &dyn CredentialStore,
    data_key: &SecretKey,
) -> Result<(), String> {
    let system_key = SecretKey::generate();
    credentials.write(DATA_KEY_ACCOUNT, &system_key.to_base64())?;
    database.write(|transaction| {
        store::save_wrap(transaction, WRAP_SYSTEM, None, &system_key, data_key)
    })
}

fn create_password_wrap(
    database: &Database,
    password: &str,
    data_key: &SecretKey,
) -> Result<(), String> {
    if password.is_empty() {
        return Err("Master password cannot be empty.".to_string());
    }
    let kdf = KdfParams::generate();
    let password_key = kdf.derive(password)?;
    database.write(|transaction| {
        store::save_wrap(
            transaction,
            WRAP_PASSWORD,
            Some(&kdf),
            &password_key,
            data_key,
        )
    })
}

impl SecretStoreState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SecretStoreRuntime::default())),
        }
    }

    fn runtime(&self) -> Result<MutexGuard<'_, SecretStoreRuntime>, String> {
        self.inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())
    }

    fn set_data_key(&self, data_key: Option<SecretKey>) -> Result<(), String> {
        let mut runtime = self.runtime()?;
        runtime.data_key = data_key;
        runtime.notice = None;
        Ok(())
    }

    fn set_notice(&self, notice: String) {
        if let Ok(mut runtime) = self.runtime() {
            runtime.notice = Some(notice);
        }
    }

    fn mode() -> Result<SecretStorageMode, String> {
        Ok(SecretStorageMode::from_config_value(
            &load_config_file()?.secret_storage_mode,
        ))
    }

    fn save_mode(mode: SecretStorageMode) -> Result<(), String> {
        let mut config = load_config_file()?;
        if config.secret_storage_mode == mode.as_str() {
            return Ok(());
        }
        // A master password is useless unless it is asked for, so turn the
        // startup prompt on when switching to it.
        if mode == SecretStorageMode::Password
            && SecretStorageMode::from_config_value(&config.secret_storage_mode) != mode
        {
            config.prompt_unlock_vault_on_startup = true;
        }
        config.secret_storage_mode = mode.as_str().to_string();
        save_config_file(&config)
    }

    pub fn keyring_available(&self) -> Result<bool, String> {
        if let Some(value) = self.runtime()?.keyring_available {
            return Ok(value);
        }
        let available = keyring_backend::probe();
        self.runtime()?.keyring_available = Some(available);
        Ok(available)
    }

    pub fn unlocked(&self) -> Result<bool, String> {
        Ok(self.runtime()?.data_key.is_some())
    }

    /// The data key, for writing many secrets in one transaction.
    pub(crate) fn data_key(&self) -> Result<SecretKey, String> {
        if Self::mode()? == SecretStorageMode::Memory {
            return Err(
                "Passwords are not saved in this storage mode. Choose another mode in Settings > Security."
                    .to_string(),
            );
        }
        self.runtime()?
            .data_key
            .clone()
            .ok_or_else(|| LOCKED.to_string())
    }

    fn unlocked_key(&self) -> Result<Option<SecretKey>, String> {
        if Self::mode()? == SecretStorageMode::Memory {
            return Ok(None);
        }
        Ok(self.runtime()?.data_key.clone())
    }

    pub fn get_status(&self) -> Result<SecretBackendStatus, String> {
        let mode = Self::mode()?;
        let keyring_available = self.keyring_available()?;
        let (unlocked, notice) = {
            let runtime = self.runtime()?;
            (runtime.data_key.is_some(), runtime.notice.clone())
        };
        // A database that failed to open reports its error instead of
        // failing the whole settings page.
        let (migrated, has_master_password, notice) = match crate::db::read(|connection| {
            Ok((
                store::migrated(connection)?,
                store::has_wrap(connection, WRAP_PASSWORD)?,
            ))
        }) {
            Ok((migrated, has_master_password)) => (migrated, has_master_password, notice),
            Err(error) => (true, false, notice.or(Some(error))),
        };
        let migration_pending = !migrated;
        let persistence_available = unlocked && mode != SecretStorageMode::Memory;
        let message = notice.or_else(|| match mode {
            _ if migration_pending => Some(NEEDS_VAULT_PASSWORD.to_string()),
            SecretStorageMode::Memory => {
                Some("Passwords are only kept for the current app session.".to_string())
            }
            SecretStorageMode::System if !keyring_available => Some(
                "The system credential store is unavailable. Use a master password instead."
                    .to_string(),
            ),
            SecretStorageMode::Password if !has_master_password => {
                Some("Set a master password to save passwords.".to_string())
            }
            _ if !unlocked => {
                Some("Enter the master password to unlock saved passwords.".to_string())
            }
            _ => None,
        });

        Ok(SecretBackendStatus {
            storage_mode: mode.as_str().to_string(),
            keyring_available,
            unlocked,
            has_master_password,
            persistence_available,
            migration_pending,
            message,
        })
    }

    /// Runs at startup: moves passwords saved by older versions into the
    /// database once, then unlocks from the credential store in `system` mode.
    pub fn initialize(&self, app: &AppHandle) {
        let result = crate::db::get().and_then(|database| {
            if database.read(store::migrated)? {
                if Self::mode()? == SecretStorageMode::System {
                    self.unlock_with_system_key(database)?;
                }
                Ok(())
            } else {
                self.migrate_legacy(app, database, None)
            }
        });
        if let Err(error) = result {
            eprintln!("Saved passwords: {error}");
            if error != NEEDS_VAULT_PASSWORD {
                self.set_notice(error);
            }
        }
    }

    fn unlock_with_system_key(&self, database: &Database) -> Result<(), String> {
        if !self.keyring_available()? {
            return Err("The system credential store is unavailable.".to_string());
        }
        match open_with_system_key(database, &OsCredentials)? {
            Some(data_key) => self.set_data_key(Some(data_key)),
            None if database.read(|connection| store::has_wrap(connection, WRAP_PASSWORD))? => {
                Err("Enter the master password to unlock saved passwords.".to_string())
            }
            None => {
                // No key at all yet (e.g. the database was reset): start over.
                let data_key = SecretKey::generate();
                database.write(|transaction| {
                    transaction
                        .execute("DELETE FROM secrets", [])
                        .map(|_| ())
                        .map_err(crate::db::sql_error("Failed to reset saved passwords"))
                })?;
                create_system_wrap(database, &OsCredentials, &data_key)?;
                self.set_data_key(Some(data_key))
            }
        }
    }

    fn migrate_legacy(
        &self,
        app: &AppHandle,
        database: &Database,
        password: Option<&str>,
    ) -> Result<(), String> {
        let secrets_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
            .join("secrets");
        let vault = legacy::LegacyVault::load(&secrets_dir)?;
        let old_mode = load_config_file()?.secret_storage_mode;
        let plaintext = crate::ssh::load_legacy_password_store()?
            .profiles
            .into_iter()
            .map(|record| (record.profile_name, Zeroizing::new(record.password)))
            .collect::<Vec<_>>();
        let mut candidate_keys = crate::profiles::saved_secret_keys()?;
        candidate_keys.extend(plaintext.iter().map(|(key, _)| key.clone()));
        candidate_keys.sort();
        candidate_keys.dedup();

        let migration = migration::migrate(
            database,
            &OsCredentials,
            LegacySources {
                old_mode: &old_mode,
                keyring_available: self.keyring_available()?,
                vault: &vault,
                password,
                plaintext,
                candidate_keys,
            },
        )?;
        eprintln!(
            "Moved {} saved passwords into the database ({} mode)",
            migration.migrated,
            migration.mode.as_str()
        );
        Self::save_mode(migration.mode)?;
        self.set_data_key(migration.data_key)?;

        // The database now holds everything; the rest is tidying up.
        let cleanup = migration.cleanup;
        for account in &cleanup.keyring_accounts {
            if let Err(error) = OsCredentials.delete(account) {
                eprintln!("Failed to delete old credential '{account}': {error}");
            }
        }
        if cleanup.retire_vault {
            if let Err(error) = vault.retire() {
                eprintln!("Failed to retire the old vault files: {error}");
            }
        }
        if cleanup.remove_plaintext {
            if let Err(error) = crate::ssh::remove_legacy_password_store() {
                eprintln!("Failed to remove the old password file: {error}");
            }
        }
        if !cleanup.unreadable_accounts.is_empty() {
            self.set_notice(format!(
                "{} saved passwords could not be read from the system credential store and were left there.",
                cleanup.unreadable_accounts.len()
            ));
        }
        Ok(())
    }

    /// Unlocks with the master password. Also finishes a migration waiting
    /// for the old vault password, sets the first master password in
    /// `password` mode, and restores the credential store key in `system` mode.
    pub fn unlock(
        &self,
        app: &AppHandle,
        input: VaultPasswordInput,
    ) -> Result<SecretBackendStatus, String> {
        let password = Zeroizing::new(input.password);
        if password.is_empty() {
            return Err("Password cannot be empty.".to_string());
        }
        let database = crate::db::get()?;
        if !database.read(store::migrated)? {
            self.migrate_legacy(app, database, Some(&password))?;
            return self.get_status();
        }

        let mode = Self::mode()?;
        let data_key = match open_with_password(database, &password)? {
            Some(data_key) => data_key,
            None if mode == SecretStorageMode::Password => {
                // First master password. Keep the current data key when there
                // is one so saved passwords stay readable.
                let data_key = match self.runtime()?.data_key.clone() {
                    Some(data_key) => data_key,
                    None => match open_with_system_key(database, &OsCredentials) {
                        Ok(Some(data_key)) => data_key,
                        _ => SecretKey::generate(),
                    },
                };
                create_password_wrap(database, &password, &data_key)?;
                data_key
            }
            None => return Err("No master password is set.".to_string()),
        };

        if mode == SecretStorageMode::System && self.keyring_available()? {
            let system_key_works =
                matches!(open_with_system_key(database, &OsCredentials), Ok(Some(_)));
            if !system_key_works {
                create_system_wrap(database, &OsCredentials, &data_key)?;
            }
        }
        self.set_data_key(Some(data_key))?;
        self.get_status()
    }

    pub fn lock(&self) -> Result<SecretBackendStatus, String> {
        self.set_data_key(None)?;
        self.get_status()
    }

    pub fn change_master_password(
        &self,
        input: ChangeVaultPasswordInput,
    ) -> Result<SecretBackendStatus, String> {
        let current = Zeroizing::new(input.current_password);
        let new = Zeroizing::new(input.new_password);
        if new.is_empty() {
            return Err("Master password cannot be empty.".to_string());
        }
        let database = crate::db::get()?;
        let data_key = open_with_password(database, &current)?
            .ok_or_else(|| "No master password is set.".to_string())
            .map_err(|error| {
                if current.is_empty() {
                    WRONG_MASTER_PASSWORD.to_string()
                } else {
                    error
                }
            })?;
        create_password_wrap(database, &new, &data_key)?;
        self.set_data_key(Some(data_key))?;
        self.get_status()
    }

    /// Adds a master password while unlocked, e.g. as a recovery password in
    /// `system` mode.
    pub fn set_master_password(&self, password: &str) -> Result<SecretBackendStatus, String> {
        let database = crate::db::get()?;
        if database.read(|connection| store::has_wrap(connection, WRAP_PASSWORD))? {
            return Err("A master password is already set. Change it instead.".to_string());
        }
        let data_key = self
            .runtime()?
            .data_key
            .clone()
            .ok_or_else(|| LOCKED.to_string())?;
        create_password_wrap(database, password, &data_key)?;
        self.get_status()
    }

    /// Drops the master password in `system` mode, which then relies on the
    /// credential store alone.
    pub fn remove_master_password(&self) -> Result<SecretBackendStatus, String> {
        if Self::mode()? != SecretStorageMode::System {
            return Err("The master password is required in this storage mode.".to_string());
        }
        let database = crate::db::get()?;
        if !database.read(|connection| store::has_wrap(connection, WRAP_SYSTEM))? {
            return Err(
                "Unlock saved passwords with the system credential store first.".to_string(),
            );
        }
        database.write(|transaction| store::delete_wrap(transaction, WRAP_PASSWORD))?;
        self.get_status()
    }

    pub fn set_storage_mode(
        &self,
        mode: &str,
        password: Option<&str>,
    ) -> Result<SecretBackendStatus, String> {
        let mode = SecretStorageMode::parse(mode)?;
        let database = crate::db::get()?;
        if !database.read(store::migrated)? {
            return Err(NEEDS_VAULT_PASSWORD.to_string());
        }
        match mode {
            SecretStorageMode::System => {
                if !self.keyring_available()? {
                    return Err("The system credential store is unavailable.".to_string());
                }
                let data_key = self.data_key_for_mode_change(database)?;
                create_system_wrap(database, &OsCredentials, &data_key)?;
                self.set_data_key(Some(data_key))?;
            }
            SecretStorageMode::Password => {
                let data_key = self.data_key_for_mode_change(database)?;
                if !database.read(|connection| store::has_wrap(connection, WRAP_PASSWORD))? {
                    let password = password
                        .filter(|password| !password.is_empty())
                        .ok_or_else(|| "Choose a master password first.".to_string())?;
                    create_password_wrap(database, password, &data_key)?;
                }
                database.write(|transaction| store::delete_wrap(transaction, WRAP_SYSTEM))?;
                if let Err(error) = OsCredentials.delete(DATA_KEY_ACCOUNT) {
                    eprintln!("Failed to delete the credential store key: {error}");
                }
                self.set_data_key(Some(data_key))?;
            }
            // Saved passwords stay in the database, unused until the mode
            // is switched back.
            SecretStorageMode::Memory => self.set_data_key(None)?,
        }
        Self::save_mode(mode)?;
        self.get_status()
    }

    fn data_key_for_mode_change(&self, database: &Database) -> Result<SecretKey, String> {
        if let Some(data_key) = self.runtime()?.data_key.clone() {
            return Ok(data_key);
        }
        if self.keyring_available()? {
            if let Ok(Some(data_key)) = open_with_system_key(database, &OsCredentials) {
                return Ok(data_key);
            }
        }
        let has_any_wrap = database.read(|connection| {
            Ok(store::has_wrap(connection, WRAP_SYSTEM)?
                || store::has_wrap(connection, WRAP_PASSWORD)?)
        })?;
        if has_any_wrap {
            Err("Enter the master password to unlock saved passwords first.".to_string())
        } else {
            Ok(SecretKey::generate())
        }
    }

    pub fn get_password(&self, _app: &AppHandle, key: &str) -> Result<Option<String>, String> {
        let Some(data_key) = self.unlocked_key()? else {
            return Ok(None);
        };
        crate::db::read(|connection| store::get_secret(connection, &data_key, key))
            .map(|value| value.map(|value| value.to_string()))
    }

    pub fn save_password(
        &self,
        _app: &AppHandle,
        key: &str,
        password: &str,
    ) -> Result<SecretLocation, String> {
        let Some(data_key) = self.unlocked_key()? else {
            return Ok(SecretLocation::Memory);
        };
        crate::db::write(|transaction| store::put_secret(transaction, &data_key, key, password))?;
        Ok(SecretLocation::Database)
    }

    /// Keys with a saved password, readable while locked.
    pub(crate) fn saved_keys(&self) -> Result<Vec<String>, String> {
        crate::db::read(store::secret_keys)
    }

    pub fn delete_password(&self, _app: &AppHandle, key: &str) -> Result<bool, String> {
        crate::db::write(|transaction| store::delete_secret(transaction, key))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, HashSet};

    /// An in-memory credential store that records reads.
    #[derive(Default)]
    pub(crate) struct MemoryCredentials {
        items: RefCell<BTreeMap<String, String>>,
        denied: RefCell<HashSet<String>>,
        reads: RefCell<Vec<String>>,
    }

    impl MemoryCredentials {
        pub(crate) fn with(items: &[(&str, &str)]) -> Self {
            let credentials = Self::default();
            for (account, value) in items {
                credentials
                    .items
                    .borrow_mut()
                    .insert(account.to_string(), value.to_string());
            }
            credentials
        }

        pub(crate) fn deny(&self, account: &str) {
            self.denied.borrow_mut().insert(account.to_string());
        }

        pub(crate) fn reads(&self) -> Vec<String> {
            self.reads.borrow().clone()
        }

        pub(crate) fn get(&self, account: &str) -> Option<String> {
            self.items.borrow().get(account).cloned()
        }
    }

    impl CredentialStore for MemoryCredentials {
        fn read(&self, account: &str) -> Result<Option<Zeroizing<String>>, String> {
            self.reads.borrow_mut().push(account.to_string());
            if self.denied.borrow().contains(account) {
                return Err("User canceled the operation.".to_string());
            }
            Ok(self.get(account).map(Zeroizing::new))
        }

        fn write(&self, account: &str, value: &str) -> Result<(), String> {
            self.items
                .borrow_mut()
                .insert(account.to_string(), value.to_string());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<bool, String> {
            Ok(self.items.borrow_mut().remove(account).is_some())
        }

        fn list_accounts(&self) -> Option<Vec<String>> {
            Some(self.items.borrow().keys().cloned().collect())
        }
    }

    #[test]
    fn a_lost_credential_store_key_is_reported_and_recoverable_by_password() {
        let database = Database::open_in_memory().unwrap();
        let credentials = MemoryCredentials::default();
        let data_key = SecretKey::generate();
        create_system_wrap(&database, &credentials, &data_key).unwrap();
        create_password_wrap(&database, "recovery", &data_key).unwrap();
        database
            .write(|c| store::put_secret(c, &data_key, "p1", "secret"))
            .unwrap();

        credentials.delete(DATA_KEY_ACCOUNT).unwrap();
        let error = open_with_system_key(&database, &credentials).unwrap_err();
        assert_eq!(error, MISSING_SYSTEM_KEY);

        credentials
            .write(DATA_KEY_ACCOUNT, &SecretKey::generate().to_base64())
            .unwrap();
        let error = open_with_system_key(&database, &credentials).unwrap_err();
        assert_eq!(error, WRONG_SYSTEM_KEY);

        let recovered = open_with_password(&database, "recovery").unwrap().unwrap();
        create_system_wrap(&database, &credentials, &recovered).unwrap();
        let reopened = open_with_system_key(&database, &credentials)
            .unwrap()
            .unwrap();
        let value = database
            .read(|c| store::get_secret(c, &reopened, "p1"))
            .unwrap()
            .unwrap();
        assert_eq!(value.as_str(), "secret");
    }

    #[test]
    fn changing_the_master_password_keeps_secrets_readable() {
        let database = Database::open_in_memory().unwrap();
        let data_key = SecretKey::generate();
        create_password_wrap(&database, "old", &data_key).unwrap();
        database
            .write(|c| store::put_secret(c, &data_key, "p1", "secret"))
            .unwrap();

        let unlocked = open_with_password(&database, "old").unwrap().unwrap();
        create_password_wrap(&database, "new", &unlocked).unwrap();

        assert_eq!(
            open_with_password(&database, "old").unwrap_err(),
            WRONG_MASTER_PASSWORD
        );
        let reopened = open_with_password(&database, "new").unwrap().unwrap();
        let value = database
            .read(|c| store::get_secret(c, &reopened, "p1"))
            .unwrap()
            .unwrap();
        assert_eq!(value.as_str(), "secret");
    }
}
