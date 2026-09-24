//! Reads the vault files that held passwords before the database. Nothing
//! here writes them; after a successful migration they are renamed away.

use super::crypto::{self, SecretKey};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub(crate) const VAULT_FILE_NAME: &str = "secret_vault.json";
pub(crate) const VAULT_CONFIG_FILE_NAME: &str = "secret_vault_config.json";
const SECRET_KIND_PASSWORD: &str = "password";
const SECRET_KIND_VERIFIER: &str = "verifier";
const VERIFIER_PROFILE_ID: &str = "__vault_verifier__";
const VERIFIER_PLAINTEXT: &str = "tterm-vault-verifier-v1";

#[derive(Debug, Deserialize)]
struct VaultConfigFile {
    #[serde(default)]
    salt_b64: String,
    #[serde(default = "default_memory_kib")]
    memory_kib: u32,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_parallelism")]
    parallelism: u32,
}

fn default_memory_kib() -> u32 {
    19_456
}

fn default_iterations() -> u32 {
    3
}

fn default_parallelism() -> u32 {
    1
}

#[derive(Debug, Default, Deserialize)]
struct VaultFile {
    #[serde(default)]
    secrets: Vec<VaultSecretRecord>,
}

#[derive(Debug, Deserialize)]
struct VaultSecretRecord {
    profile_id: String,
    kind: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

pub(crate) struct LegacyVault {
    directory: PathBuf,
    config: Option<VaultConfigFile>,
    file: VaultFile,
}

impl LegacyVault {
    /// `directory` is the app data `secrets` directory.
    pub(crate) fn load(directory: &Path) -> Result<Self, String> {
        let config_path = directory.join(VAULT_CONFIG_FILE_NAME);
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .map_err(|e| format!("Failed to read vault config: {e}"))?;
            Some(
                serde_json::from_str(&content)
                    .map_err(|e| format!("Failed to parse vault config: {e}"))?,
            )
        } else {
            None
        };
        let vault_path = directory.join(VAULT_FILE_NAME);
        let file = if vault_path.exists() {
            let content = fs::read_to_string(&vault_path)
                .map_err(|e| format!("Failed to read vault: {e}"))?;
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse vault: {e}"))?
        } else {
            VaultFile::default()
        };
        Ok(Self {
            directory: directory.to_path_buf(),
            config,
            file,
        })
    }

    /// Whether the vault holds any saved password.
    pub(crate) fn has_passwords(&self) -> bool {
        self.file
            .secrets
            .iter()
            .any(|record| record.kind == SECRET_KIND_PASSWORD)
    }

    /// Decrypts every password. Fails when `password` is not the vault's.
    pub(crate) fn decrypt_all(
        &self,
        password: &str,
    ) -> Result<Vec<(String, Zeroizing<String>)>, String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| "The app vault settings file is missing.".to_string())?;
        let salt = BASE64
            .decode(config.salt_b64.as_bytes())
            .map_err(|e| format!("Failed to decode vault salt: {e}"))?;
        let key = crypto::derive_argon2id(
            password.as_bytes(),
            &salt,
            config.memory_kib,
            config.iterations,
            config.parallelism,
        )?;

        let verifier = self.file.secrets.iter().find(|record| {
            record.profile_id == VERIFIER_PROFILE_ID && record.kind == SECRET_KIND_VERIFIER
        });
        if let Some(record) = verifier {
            if decrypt(&key, record)?.as_str() != VERIFIER_PLAINTEXT {
                return Err(wrong_password());
            }
        }

        let mut passwords = Vec::new();
        for record in &self.file.secrets {
            if record.kind == SECRET_KIND_PASSWORD {
                passwords.push((record.profile_id.clone(), decrypt(&key, record)?));
            }
        }
        Ok(passwords)
    }

    /// Renames the vault files to `*.migrated`, keeping them for recovery.
    pub(crate) fn retire(&self) -> Result<(), String> {
        for name in [VAULT_FILE_NAME, VAULT_CONFIG_FILE_NAME] {
            let path = self.directory.join(name);
            if path.exists() {
                let target = self.directory.join(format!("{name}.migrated"));
                fs::rename(&path, &target)
                    .map_err(|e| format!("Failed to rename '{}': {e}", path.display()))?;
            }
        }
        Ok(())
    }
}

fn wrong_password() -> String {
    "Failed to unlock the app vault. Check the vault password.".to_string()
}

fn decrypt(key: &SecretKey, record: &VaultSecretRecord) -> Result<Zeroizing<String>, String> {
    let nonce = BASE64
        .decode(record.nonce_b64.as_bytes())
        .map_err(|e| format!("Failed to decode vault nonce: {e}"))?;
    let ciphertext = BASE64
        .decode(record.ciphertext_b64.as_bytes())
        .map_err(|e| format!("Failed to decode vault secret: {e}"))?;
    if nonce.len() != crypto::NONCE_LEN {
        return Err("Vault record has an invalid nonce".to_string());
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.expose()));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map(Zeroizing::new)
        .map_err(|_| wrong_password())?;
    String::from_utf8(plaintext.to_vec())
        .map(Zeroizing::new)
        .map_err(|e| format!("Vault secret is not valid UTF-8: {e}"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rand::RngCore;

    /// Writes a vault in the pre-database format, as the old code did.
    pub(crate) fn write_legacy_vault(directory: &Path, password: &str, secrets: &[(&str, &str)]) {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        // Cheap parameters keep the tests fast; the reader honors the file.
        let (memory_kib, iterations, parallelism) = (64, 1, 1);
        fs::create_dir_all(directory).unwrap();
        fs::write(
            directory.join(VAULT_CONFIG_FILE_NAME),
            serde_json::json!({
                "salt_b64": BASE64.encode(salt),
                "version": 1,
                "algorithm": "AES-256-GCM",
                "kdf": "Argon2id-v1.3",
                "memory_kib": memory_kib,
                "iterations": iterations,
                "parallelism": parallelism,
            })
            .to_string(),
        )
        .unwrap();
        let key = crypto::derive_argon2id(
            password.as_bytes(),
            &salt,
            memory_kib,
            iterations,
            parallelism,
        )
        .unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.expose()));
        let record = |profile_id: &str, kind: &str, plaintext: &str| {
            let mut nonce = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut nonce);
            let ciphertext = cipher
                .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
                .unwrap();
            serde_json::json!({
                "profile_id": profile_id,
                "kind": kind,
                "nonce_b64": BASE64.encode(nonce),
                "ciphertext_b64": BASE64.encode(ciphertext),
                "updated_at": 1,
            })
        };
        let mut records = vec![record(
            VERIFIER_PROFILE_ID,
            SECRET_KIND_VERIFIER,
            VERIFIER_PLAINTEXT,
        )];
        records.extend(
            secrets
                .iter()
                .map(|(id, value)| record(id, SECRET_KIND_PASSWORD, value)),
        );
        fs::write(
            directory.join(VAULT_FILE_NAME),
            serde_json::json!({ "secrets": records }).to_string(),
        )
        .unwrap();
    }

    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tterm-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn decrypts_a_legacy_vault_and_rejects_a_wrong_password() {
        let dir = temp_dir("legacy-vault");
        write_legacy_vault(
            &dir,
            "vault-pass",
            &[("p1", "one"), ("p2:jump:h:22:u", "two")],
        );
        let vault = LegacyVault::load(&dir).unwrap();
        assert!(vault.has_passwords());
        let passwords = vault.decrypt_all("vault-pass").unwrap();
        let plain: Vec<_> = passwords
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(plain, [("p1", "one"), ("p2:jump:h:22:u", "two")]);
        assert!(vault.decrypt_all("wrong").is_err());

        vault.retire().unwrap();
        assert!(!dir.join(VAULT_FILE_NAME).exists());
        assert!(dir.join(format!("{VAULT_FILE_NAME}.migrated")).exists());
        assert!(!LegacyVault::load(&dir).unwrap().has_passwords());
        fs::remove_dir_all(dir).ok();
    }
}
