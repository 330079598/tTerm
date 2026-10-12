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
fn resetting_drops_unreadable_secrets_and_unlocks_with_the_credential_store() {
    let database = Database::open_in_memory().unwrap();
    let credentials = MemoryCredentials::default();
    let lost_key = SecretKey::generate();
    create_system_wrap(&database, &credentials, &lost_key).unwrap();
    create_password_wrap(&database, "forgotten", &lost_key).unwrap();
    database
        .write(|c| store::put_secret(c, &lost_key, "p1", "secret"))
        .unwrap();
    // The database came from another computer, whose key is not here.
    credentials
        .write(DATA_KEY_ACCOUNT, &SecretKey::generate().to_base64())
        .unwrap();
    assert_eq!(
        open_with_system_key(&database, &credentials).unwrap_err(),
        WRONG_SYSTEM_KEY
    );

    let data_key = reset_with_system_key(&database, &credentials).unwrap();

    let reopened = open_with_system_key(&database, &credentials)
        .unwrap()
        .unwrap();
    assert_eq!(reopened.to_base64(), data_key.to_base64());
    assert!(database.read(store::secret_keys).unwrap().is_empty());
    assert!(!database
        .read(|c| store::has_wrap(c, WRAP_PASSWORD))
        .unwrap());
}

#[test]
fn resetting_keeps_everything_when_the_credential_store_fails() {
    let database = Database::open_in_memory().unwrap();
    let credentials = MemoryCredentials::default();
    let data_key = SecretKey::generate();
    create_system_wrap(&database, &credentials, &data_key).unwrap();
    database
        .write(|c| store::put_secret(c, &data_key, "p1", "secret"))
        .unwrap();
    credentials.deny(DATA_KEY_ACCOUNT);

    assert!(reset_with_system_key(&database, &credentials).is_err());
    assert_eq!(database.read(store::secret_keys).unwrap(), vec!["p1"]);
}

#[test]
fn a_second_database_keeps_the_credential_store_key_of_the_first() {
    let credentials = MemoryCredentials::default();
    let first = Database::open_in_memory().unwrap();
    let first_key = SecretKey::generate();
    create_system_wrap(&first, &credentials, &first_key).unwrap();
    let stored = credentials.get(DATA_KEY_ACCOUNT).unwrap();

    let second = Database::open_in_memory().unwrap();
    create_system_wrap(&second, &credentials, &SecretKey::generate()).unwrap();

    assert_eq!(credentials.get(DATA_KEY_ACCOUNT).unwrap(), stored);
    assert!(open_with_system_key(&first, &credentials)
        .unwrap()
        .is_some());
    assert!(open_with_system_key(&second, &credentials)
        .unwrap()
        .is_some());
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
