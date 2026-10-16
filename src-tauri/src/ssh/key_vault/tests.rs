use super::entry::{inspect, VaultKey};
use super::import::{ensure_new, read_import};
use super::{export, reference_id, store, ImportVaultKeyInput, REFERENCE_PREFIX};
use crate::db::Database;
use crate::profiles::types::SavedProfile;
use crate::ssh::secret_store::crypto::SecretKey as DataKey;
use russh::keys::ssh_key::certificate::{Builder, CertType};
use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};
use russh::keys::{Certificate, HashAlg, PublicKey};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty folder for one test's files.
fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tterm-key-vault-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn new_key(passphrase: Option<&str>) -> PrivateKey {
    let mut rng = rand::rng();
    let key = PrivateKey::random(&mut rng, Algorithm::Ed25519).unwrap();
    match passphrase {
        Some(passphrase) => key.encrypt(&mut rng, passphrase).unwrap(),
        None => key,
    }
}

fn key_text(key: &PrivateKey) -> String {
    key.to_openssh(LineEnding::LF).unwrap().to_string()
}

fn write_key(dir: &Path, name: &str, key: &PrivateKey) -> String {
    let path = dir.join(name);
    std::fs::write(&path, key_text(key)).unwrap();
    path.to_string_lossy().into_owned()
}

fn certificate_for(key: &PublicKey, valid_before: u64) -> Certificate {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ca = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
    let mut builder =
        Builder::new_with_random_nonce(&mut rand::rng(), key, now - 60, valid_before).unwrap();
    builder.cert_type(CertType::User).unwrap();
    builder.valid_principal("root").unwrap();
    builder.sign(&ca).unwrap()
}

fn import_input(path: &str) -> ImportVaultKeyInput {
    ImportVaultKeyInput {
        path: path.to_string(),
        name: None,
        passphrase: None,
        remember_passphrase: false,
        certificate_path: None,
    }
}

#[test]
fn reference_names_the_key_id() {
    assert_eq!(reference_id("tterm-key:abc"), Some("abc"));
    assert_eq!(reference_id("  tterm-key: abc "), Some("abc"));
    assert_eq!(reference_id("tterm-key:"), None);
    assert_eq!(reference_id("/home/me/.ssh/id_ed25519"), None);
    assert_eq!(reference_id("C:\\keys\\tterm-key:abc"), None);
}

#[test]
fn inspect_reads_the_public_half_and_checks_a_passphrase() {
    let plain = new_key(None);
    let info = inspect(&key_text(&plain), None).unwrap();
    assert_eq!(&info.public_key, plain.public_key());
    assert!(!info.encrypted);

    let locked = new_key(Some("secret"));
    let text = key_text(&locked);
    // OpenSSH keys show their public half without the passphrase.
    assert!(inspect(&text, None).unwrap().encrypted);
    assert!(inspect(&text, Some("secret")).unwrap().encrypted);
    assert_eq!(
        inspect(&text, Some("wrong")).err().unwrap(),
        "That passphrase does not unlock the key."
    );

    let error = inspect("not a key", None).err().unwrap();
    assert!(
        error.starts_with("The file is not a private key"),
        "{error}"
    );
}

#[test]
fn import_names_the_key_after_its_file_and_keeps_a_passphrase_only_when_asked() {
    let dir = temp_dir();
    let locked = new_key(Some("secret"));
    let path = write_key(&dir, "id_work", &locked);

    let key = read_import(&ImportVaultKeyInput {
        passphrase: Some("secret".to_string()),
        ..import_input(&path)
    })
    .unwrap();
    assert_eq!(key.name, "id_work");
    assert_eq!(key.private_key, key_text(&locked));
    assert_eq!(key.passphrase, None);
    assert_eq!(
        PublicKey::from_openssh(&key.public_key).unwrap(),
        *locked.public_key()
    );

    let kept = read_import(&ImportVaultKeyInput {
        name: Some("  Work laptop ".to_string()),
        passphrase: Some("secret".to_string()),
        remember_passphrase: true,
        ..import_input(&path)
    })
    .unwrap();
    assert_eq!(kept.name, "Work laptop");
    assert_eq!(kept.passphrase.as_deref(), Some("secret"));

    // A key without a passphrase has none to keep.
    let plain_path = write_key(&dir, "id_plain", &new_key(None));
    let plain = read_import(&ImportVaultKeyInput {
        passphrase: Some("ignored".to_string()),
        remember_passphrase: true,
        ..import_input(&plain_path)
    })
    .unwrap();
    assert_eq!(plain.passphrase, None);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn import_takes_only_a_certificate_for_the_key() {
    let dir = temp_dir();
    let key = new_key(None);
    let path = write_key(&dir, "id", &key);
    let far = u64::MAX;

    let own = dir.join("own-cert.pub");
    let certificate = certificate_for(key.public_key(), far);
    std::fs::write(&own, certificate.to_openssh().unwrap()).unwrap();
    let imported = read_import(&ImportVaultKeyInput {
        certificate_path: Some(own.to_string_lossy().into_owned()),
        ..import_input(&path)
    })
    .unwrap();
    assert_eq!(
        imported.certificate.as_deref(),
        Some(certificate.to_openssh().unwrap().trim())
    );

    let other = dir.join("other-cert.pub");
    let foreign = certificate_for(new_key(None).public_key(), far);
    std::fs::write(&other, foreign.to_openssh().unwrap()).unwrap();
    let error = read_import(&ImportVaultKeyInput {
        certificate_path: Some(other.to_string_lossy().into_owned()),
        ..import_input(&path)
    })
    .err()
    .unwrap();
    assert!(error.contains("certifies a different key"), "{error}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn import_refuses_missing_and_oversized_files() {
    let dir = temp_dir();
    let missing = dir.join("absent").to_string_lossy().into_owned();
    let error = read_import(&import_input(&missing)).err().unwrap();
    assert!(error.starts_with("The key file was not found"), "{error}");

    let big = dir.join("big");
    std::fs::write(&big, vec![b'a'; 65 * 1024]).unwrap();
    let error = read_import(&import_input(&big.to_string_lossy()))
        .err()
        .unwrap();
    assert!(error.ends_with("is too big to be an SSH key."), "{error}");
    std::fs::remove_dir_all(dir).ok();
}

fn vault_key(name: &str, key: &PrivateKey) -> VaultKey {
    VaultKey {
        name: name.to_string(),
        private_key: key_text(key),
        public_key: key.public_key().to_openssh().unwrap(),
        passphrase: None,
        certificate: None,
        created_at: 1,
    }
}

#[test]
fn keys_are_saved_secrets_that_stay_apart_from_passwords() {
    let database = Database::open_in_memory().unwrap();
    let data_key = DataKey::generate();
    let key = new_key(None);
    database
        .write(|connection| {
            crate::ssh::secret_store::store::put_secret(connection, &data_key, "p1", "password")?;
            store::put(connection, &data_key, "k1", &vault_key("Laptop", &key))?;
            assert_eq!(store::ids(connection)?, ["k1"]);
            assert!(store::exists(connection, "k1")?);
            assert!(!store::exists(connection, "p1")?);

            let saved = store::get(connection, &data_key, "k1")?.unwrap();
            assert_eq!(saved.name, "Laptop");
            assert_eq!(saved.private_key, key_text(&key));

            // The same key cannot come in twice, whatever it is called.
            let error = ensure_new(connection, &data_key, &vault_key("Again", &key))
                .err()
                .unwrap();
            assert_eq!(error, "This key is already in the key vault as \"Laptop\".");
            ensure_new(connection, &data_key, &vault_key("Other", &new_key(None)))?;

            assert!(store::delete(connection, "k1")?);
            assert!(store::ids(connection)?.is_empty());
            Ok(())
        })
        .unwrap();
}

#[test]
fn summary_describes_the_key() {
    let key = new_key(Some("secret"));
    let mut saved = vault_key("Laptop", &key);
    saved.passphrase = Some("secret".to_string());
    saved.certificate = Some(
        certificate_for(key.public_key(), 4_000_000_000)
            .to_openssh()
            .unwrap(),
    );
    let summary = saved.summary("k1", vec!["web".to_string()]);
    assert_eq!(summary.id, "k1");
    assert_eq!(summary.algorithm, "ssh-ed25519");
    assert_eq!(
        summary.fingerprint,
        key.public_key().fingerprint(HashAlg::Sha256).to_string()
    );
    assert!(summary.encrypted);
    assert!(summary.passphrase_saved);
    assert!(summary.has_certificate);
    assert_eq!(summary.certificate_valid_before, Some(4_000_000_000));
    assert_eq!(summary.used_by, ["web"]);
}

#[test]
fn used_by_lists_profiles_that_sign_in_with_the_key() {
    let reference = format!("{REFERENCE_PREFIX}k1");
    let profile = |name: &str, auth: &str, key: &str| {
        serde_json::json!({
            "id": name, "name": name, "connection_type": "ssh", "host": "example.com",
            "port": 22, "username": "root", "auth_method": auth, "private_key_path": key
        })
    };
    let mut through_jump = profile("via-jump", "password", "");
    through_jump["jump_hosts"] = serde_json::json!([{
        "host": "bastion", "port": 22, "username": "root",
        "auth_method": "key", "private_key_path": reference
    }]);
    let mut jump_off = through_jump.clone();
    jump_off["name"] = "jump-off".into();
    jump_off["use_jump_host"] = false.into();
    let profiles: Vec<SavedProfile> = serde_json::from_value(serde_json::json!([
        profile("direct", "key", &reference),
        profile("other-key", "key", "tterm-key:k2"),
        profile("file", "key", "/keys/id"),
        profile("stale", "password", &reference),
        through_jump,
        jump_off,
    ]))
    .unwrap();
    assert_eq!(store::used_by(&profiles, "k1"), ["direct", "via-jump"]);
}

#[test]
fn export_writes_the_key_and_its_certificate_beside_it() {
    let dir = temp_dir();
    let key = new_key(None);
    let mut saved = vault_key("Laptop", &key);
    saved.certificate = Some(
        certificate_for(key.public_key(), u64::MAX)
            .to_openssh()
            .unwrap(),
    );
    let path = dir.join("id_laptop");
    export::write_key_files(&path, &saved).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), key_text(&key));
    let certificate = std::fs::read_to_string(export::certificate_path(&path)).unwrap();
    assert_eq!(certificate.trim(), saved.certificate.as_deref().unwrap());
    assert!(export::certificate_path(&path).ends_with("id_laptop-cert.pub"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    std::fs::remove_dir_all(dir).ok();
}
