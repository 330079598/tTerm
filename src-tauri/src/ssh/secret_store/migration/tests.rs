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
