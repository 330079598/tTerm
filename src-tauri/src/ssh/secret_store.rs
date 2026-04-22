use crate::config::{load_config_file, save_config_file};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use zeroize::Zeroize;

const SERVICE_NAME: &str = "tterm";
const VAULT_FILE_NAME: &str = "secret_vault.json";
const VAULT_CONFIG_FILE_NAME: &str = "secret_vault_config.json";
const SECRET_KIND_PASSWORD: &str = "password";
const PROBE_ACCOUNT: &str = "__probe__";
const KEYRING_PROBE_SECRET: &str = "tterm-keyring-probe";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const DERIVED_KEY_LEN: usize = 32;
const PBKDF_ITERATIONS: u32 = 3;
const PBKDF_MEMORY_KIB: u32 = 19_456;
const PBKDF_PARALLELISM: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretBackendStatus {
    pub active_backend: String,
    pub keyring_available: bool,
    pub stronghold_enabled: bool,
    pub stronghold_unlocked: bool,
    pub persistence_available: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrongholdPasswordInput {
    pub password: String,
    #[serde(default)]
    pub enable_vault: bool,
}

#[derive(Debug, Clone)]
pub struct SecretStoreState {
    inner: Arc<Mutex<SecretStoreRuntime>>,
}

#[derive(Debug, Default)]
struct SecretStoreRuntime {
    cached_keyring_available: Option<bool>,
    stronghold: Option<StrongholdRuntime>,
}

#[derive(Debug)]
struct StrongholdRuntime {
    key: [u8; DERIVED_KEY_LEN],
}

#[derive(Debug, Clone)]
pub enum SecretLocation {
    Keyring,
    Stronghold,
    Memory,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VaultConfigFile {
    #[serde(default)]
    salt_b64: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VaultFile {
    #[serde(default)]
    secrets: Vec<VaultSecretRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct VaultSecretRecord {
    profile_id: String,
    kind: String,
    nonce_b64: String,
    ciphertext_b64: String,
    updated_at: i64,
}

impl SecretStoreState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SecretStoreRuntime::default())),
        }
    }

    pub fn get_status(&self) -> Result<SecretBackendStatus, String> {
        let stronghold_unlocked = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?
            .stronghold
            .is_some();
        let keyring_available = self.keyring_available()?;
        let config = load_config_file()?;
        let active_backend = if keyring_available {
            "system".to_string()
        } else if config.secret_vault_enabled && stronghold_unlocked {
            "vault".to_string()
        } else {
            "memory".to_string()
        };
        let persistence_available =
            keyring_available || (config.secret_vault_enabled && stronghold_unlocked);
        let message = if keyring_available {
            None
        } else if config.secret_vault_enabled && !stronghold_unlocked {
            Some("System keyring unavailable. Unlock the app vault to persist secrets.".to_string())
        } else if !config.secret_vault_enabled {
            Some(
                "System keyring unavailable. Enable and unlock the app vault to persist secrets."
                    .to_string(),
            )
        } else {
            Some(
                "Secrets are only kept for this session until the app vault is unlocked."
                    .to_string(),
            )
        };

        Ok(SecretBackendStatus {
            active_backend,
            keyring_available,
            stronghold_enabled: config.secret_vault_enabled,
            stronghold_unlocked,
            persistence_available,
            message,
        })
    }

    pub fn keyring_available(&self) -> Result<bool, String> {
        {
            let guard = self
                .inner
                .lock()
                .map_err(|_| "Secret store state is poisoned".to_string())?;
            if let Some(value) = guard.cached_keyring_available {
                return Ok(value);
            }
        }

        let available = Self::probe_keyring();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        guard.cached_keyring_available = Some(available);
        Ok(available)
    }

    fn probe_keyring() -> bool {
        let entry = match keyring::Entry::new(SERVICE_NAME, PROBE_ACCOUNT) {
            Ok(entry) => entry,
            Err(_) => return false,
        };

        if entry.set_password(KEYRING_PROBE_SECRET).is_err() {
            return false;
        }

        let ok = matches!(entry.get_password(), Ok(value) if value == KEYRING_PROBE_SECRET);
        let _ = entry.delete_credential();
        ok
    }

    pub fn unlock_stronghold(
        &self,
        app: &AppHandle,
        input: StrongholdPasswordInput,
    ) -> Result<SecretBackendStatus, String> {
        if input.password.is_empty() {
            return Err("Vault password cannot be empty".to_string());
        }

        let mut config = load_config_file()?;
        if input.enable_vault && !config.secret_vault_enabled {
            config.secret_vault_enabled = true;
            save_config_file(&config)?;
        }

        if !config.secret_vault_enabled {
            return Err(
                "Vault fallback is disabled. Enable it first before unlocking.".to_string(),
            );
        }

        let key = derive_or_initialize_vault_key(app, input.password.as_bytes())?;
        let runtime = StrongholdRuntime { key };

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        guard.stronghold = Some(runtime);
        drop(guard);

        self.get_status()
    }

    pub fn lock_stronghold(&self) -> Result<SecretBackendStatus, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &mut guard.stronghold {
            runtime.key.zeroize();
        }
        guard.stronghold = None;
        drop(guard);
        self.get_status()
    }

    pub fn set_vault_enabled(&self, enabled: bool) -> Result<SecretBackendStatus, String> {
        let mut config = load_config_file()?;
        config.secret_vault_enabled = enabled;
        save_config_file(&config)?;

        if !enabled {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| "Secret store state is poisoned".to_string())?;
            if let Some(runtime) = &mut guard.stronghold {
                runtime.key.zeroize();
            }
            guard.stronghold = None;
        }

        self.get_status()
    }

    pub fn get_password(
        &self,
        app: &AppHandle,
        profile_id: &str,
    ) -> Result<Option<String>, String> {
        if self.keyring_available()? {
            match read_keyring_secret(profile_id, SECRET_KIND_PASSWORD) {
                Ok(value) => return Ok(value),
                Err(err) => {
                    return Err(format!("Failed to read system credential store: {}", err));
                }
            }
        }

        let guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &guard.stronghold {
            return read_vault_secret(app, runtime, profile_id, SECRET_KIND_PASSWORD);
        }

        Ok(None)
    }

    pub fn save_password(
        &self,
        app: &AppHandle,
        profile_id: &str,
        password: &str,
    ) -> Result<SecretLocation, String> {
        if self.keyring_available()? {
            write_keyring_secret(profile_id, SECRET_KIND_PASSWORD, password)?;
            return Ok(SecretLocation::Keyring);
        }

        let guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &guard.stronghold {
            write_vault_secret(app, runtime, profile_id, SECRET_KIND_PASSWORD, password)?;
            return Ok(SecretLocation::Stronghold);
        }

        Ok(SecretLocation::Memory)
    }
}

impl Drop for SecretStoreRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = &mut self.stronghold {
            runtime.key.zeroize();
        }
    }
}

fn stronghold_key_name(profile_id: &str, kind: &str) -> String {
    format!("{}::{}", kind, profile_id)
}

fn read_keyring_secret(profile_id: &str, kind: &str) -> Result<Option<String>, keyring::Error> {
    let account = stronghold_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err),
    }
}

fn write_keyring_secret(profile_id: &str, kind: &str, value: &str) -> Result<(), String> {
    let account = stronghold_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .map_err(|e| format!("Failed to open keyring entry: {}", e))?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to write keyring secret: {}", e))
}

fn derive_or_initialize_vault_key(
    app: &AppHandle,
    password: &[u8],
) -> Result<[u8; DERIVED_KEY_LEN], String> {
    let config_path = vault_config_path(app)?;
    let config = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read vault config: {}", e))?;
        serde_json::from_str::<VaultConfigFile>(&content)
            .map_err(|e| format!("Failed to parse vault config: {}", e))?
    } else {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let config = VaultConfigFile {
            salt_b64: BASE64.encode(salt),
        };
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize vault config: {}", e))?;
        fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write vault config: {}", e))?;
        config
    };

    let salt = BASE64
        .decode(config.salt_b64.as_bytes())
        .map_err(|e| format!("Failed to decode vault salt: {}", e))?;
    let params = Params::new(
        PBKDF_MEMORY_KIB,
        PBKDF_ITERATIONS,
        PBKDF_PARALLELISM,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|e| format!("Failed to build Argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut derived = [0u8; DERIVED_KEY_LEN];
    argon2
        .hash_password_into(password, &salt, &mut derived)
        .map_err(|e| format!("Failed to derive vault key: {}", e))?;
    Ok(derived)
}

fn app_secret_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    dir.push("secrets");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create secrets dir: {}", e))?;
    }
    Ok(dir)
}

fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_secret_dir(app)?.join(VAULT_FILE_NAME))
}

fn vault_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_secret_dir(app)?.join(VAULT_CONFIG_FILE_NAME))
}

fn load_vault_file(path: &Path) -> Result<VaultFile, String> {
    if !path.exists() {
        return Ok(VaultFile::default());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read vault: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse vault: {}", e))
}

fn save_vault_file(path: &Path, vault: &VaultFile) -> Result<(), String> {
    let content = serde_json::to_string_pretty(vault)
        .map_err(|e| format!("Failed to serialize vault: {}", e))?;
    fs::write(path, content).map_err(|e| format!("Failed to write vault: {}", e))
}

fn encrypt_secret(
    runtime: &StrongholdRuntime,
    plaintext: &str,
) -> Result<(String, String), String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&runtime.key));
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| "Failed to encrypt vault secret".to_string())?;
    Ok((BASE64.encode(nonce), BASE64.encode(ciphertext)))
}

fn decrypt_secret(
    runtime: &StrongholdRuntime,
    record: &VaultSecretRecord,
) -> Result<String, String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&runtime.key));
    let nonce = BASE64
        .decode(record.nonce_b64.as_bytes())
        .map_err(|e| format!("Failed to decode vault nonce: {}", e))?;
    let ciphertext = BASE64
        .decode(record.ciphertext_b64.as_bytes())
        .map_err(|e| format!("Failed to decode vault secret: {}", e))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "Failed to decrypt vault secret. Check the vault password.".to_string())?;
    String::from_utf8(plaintext).map_err(|e| format!("Vault secret is not valid UTF-8: {}", e))
}

fn read_vault_secret(
    app: &AppHandle,
    runtime: &StrongholdRuntime,
    profile_id: &str,
    kind: &str,
) -> Result<Option<String>, String> {
    let path = vault_path(app)?;
    let vault = load_vault_file(&path)?;
    if let Some(record) = vault
        .secrets
        .iter()
        .find(|record| record.profile_id == profile_id && record.kind == kind)
    {
        return decrypt_secret(runtime, record).map(Some);
    }
    Ok(None)
}

fn write_vault_secret(
    app: &AppHandle,
    runtime: &StrongholdRuntime,
    profile_id: &str,
    kind: &str,
    plaintext: &str,
) -> Result<(), String> {
    let path = vault_path(app)?;
    let mut vault = load_vault_file(&path)?;
    let (nonce_b64, ciphertext_b64) = encrypt_secret(runtime, plaintext)?;
    if let Some(record) = vault
        .secrets
        .iter_mut()
        .find(|record| record.profile_id == profile_id && record.kind == kind)
    {
        record.nonce_b64 = nonce_b64;
        record.ciphertext_b64 = ciphertext_b64;
        record.updated_at = crate::ssh::now_unix_ms();
    } else {
        vault.secrets.push(VaultSecretRecord {
            profile_id: profile_id.to_string(),
            kind: kind.to_string(),
            nonce_b64,
            ciphertext_b64,
            updated_at: crate::ssh::now_unix_ms(),
        });
    }
    save_vault_file(&path, &vault)
}
