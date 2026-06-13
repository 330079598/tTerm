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
const SECRET_KIND_VERIFIER: &str = "verifier";
const VERIFIER_PROFILE_ID: &str = "__vault_verifier__";
const VAULT_VERIFIER_PLAINTEXT: &str = "tterm-vault-verifier-v1";
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
    pub storage_mode: String,
    pub keyring_available: bool,
    pub vault_enabled: bool,
    pub vault_unlocked: bool,
    pub persistence_available: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultPasswordInput {
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
    vault: Option<VaultRuntime>,
}

#[derive(Debug)]
struct VaultRuntime {
    key: [u8; DERIVED_KEY_LEN],
}

#[derive(Debug, Clone)]
pub enum SecretLocation {
    Keyring,
    Vault,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecretStorageMode {
    Auto,
    System,
    Vault,
    Memory,
}

impl SecretStorageMode {
    fn from_config_value(value: &str) -> Self {
        match value {
            "system" => Self::System,
            "vault" => Self::Vault,
            "memory" => Self::Memory,
            _ => Self::Auto,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::System => "system",
            Self::Vault => "vault",
            Self::Memory => "memory",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VaultConfigFile {
    #[serde(default)]
    salt_b64: String,
    #[serde(default = "default_vault_config_version")]
    version: u16,
    #[serde(default = "default_vault_algorithm")]
    algorithm: String,
    #[serde(default = "default_vault_kdf")]
    kdf: String,
    #[serde(default = "default_vault_memory_kib")]
    memory_kib: u32,
    #[serde(default = "default_vault_iterations")]
    iterations: u32,
    #[serde(default = "default_vault_parallelism")]
    parallelism: u32,
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

fn default_vault_config_version() -> u16 {
    1
}

fn default_vault_algorithm() -> String {
    "AES-256-GCM".to_string()
}

fn default_vault_kdf() -> String {
    "Argon2id-v1.3".to_string()
}

fn default_vault_memory_kib() -> u32 {
    PBKDF_MEMORY_KIB
}

fn default_vault_iterations() -> u32 {
    PBKDF_ITERATIONS
}

fn default_vault_parallelism() -> u32 {
    PBKDF_PARALLELISM
}

impl SecretStoreState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SecretStoreRuntime::default())),
        }
    }

    pub fn get_status(&self) -> Result<SecretBackendStatus, String> {
        let vault_unlocked = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?
            .vault
            .is_some();
        let keyring_available = self.keyring_available()?;
        let config = load_config_file()?;
        let mode = SecretStorageMode::from_config_value(&config.secret_storage_mode);
        let active_backend = match mode {
            SecretStorageMode::System if keyring_available => "system",
            SecretStorageMode::Vault if config.secret_vault_enabled && vault_unlocked => {
                "vault"
            }
            SecretStorageMode::Auto if keyring_available => "system",
            SecretStorageMode::Auto if config.secret_vault_enabled && vault_unlocked => {
                "vault"
            }
            _ => "memory",
        };
        let persistence_available = match mode {
            SecretStorageMode::System => keyring_available,
            SecretStorageMode::Vault => config.secret_vault_enabled && vault_unlocked,
            SecretStorageMode::Memory => false,
            SecretStorageMode::Auto => {
                keyring_available || (config.secret_vault_enabled && vault_unlocked)
            }
        };
        let message = match mode {
            SecretStorageMode::System if !keyring_available => {
                Some("System credential store is unavailable.".to_string())
            }
            SecretStorageMode::Vault if !config.secret_vault_enabled => Some(
                "App vault mode is selected. Unlock the vault before saving passwords.".to_string(),
            ),
            SecretStorageMode::Vault if !vault_unlocked => {
                Some("App vault mode is selected. Unlock the vault to persist secrets.".to_string())
            }
            SecretStorageMode::Memory => {
                Some("Passwords are only kept for the current app session.".to_string())
            }
            SecretStorageMode::Auto
                if !keyring_available && config.secret_vault_enabled && !vault_unlocked =>
            {
                Some(
                    "System keyring unavailable. Unlock the app vault to persist secrets."
                        .to_string(),
                )
            }
            SecretStorageMode::Auto if !keyring_available && !config.secret_vault_enabled => Some(
                "System keyring unavailable. Enable and unlock the app vault to persist secrets."
                    .to_string(),
            ),
            _ => None,
        };

        Ok(SecretBackendStatus {
            active_backend: active_backend.to_string(),
            storage_mode: mode.as_str().to_string(),
            keyring_available,
            vault_enabled: config.secret_vault_enabled,
            vault_unlocked,
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

    pub fn unlock_vault(
        &self,
        app: &AppHandle,
        input: VaultPasswordInput,
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
        let runtime = VaultRuntime { key };
        verify_or_initialize_vault(app, &runtime)?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        guard.vault = Some(runtime);
        drop(guard);

        self.get_status()
    }

    pub fn lock_vault(&self) -> Result<SecretBackendStatus, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &mut guard.vault {
            runtime.key.zeroize();
        }
        guard.vault = None;
        drop(guard);
        self.get_status()
    }

    pub fn set_vault_enabled(&self, enabled: bool) -> Result<SecretBackendStatus, String> {
        let mut config = load_config_file()?;
        config.secret_vault_enabled = enabled;
        if enabled && config.secret_storage_mode == "memory" {
            config.secret_storage_mode = "auto".to_string();
        }
        if !enabled && config.secret_storage_mode == "vault" {
            config.secret_storage_mode = "auto".to_string();
        }
        save_config_file(&config)?;

        if !enabled {
            let mut guard = self
                .inner
                .lock()
                .map_err(|_| "Secret store state is poisoned".to_string())?;
            if let Some(runtime) = &mut guard.vault {
                runtime.key.zeroize();
            }
            guard.vault = None;
        }

        self.get_status()
    }

    pub fn set_storage_mode(&self, mode: &str) -> Result<SecretBackendStatus, String> {
        let mode = SecretStorageMode::from_config_value(mode);
        let mut config = load_config_file()?;
        config.secret_storage_mode = mode.as_str().to_string();
        config.secret_vault_enabled =
            config.secret_vault_enabled || mode == SecretStorageMode::Vault;
        save_config_file(&config)?;
        self.get_status()
    }

    pub fn vault_unlocked(&self) -> Result<bool, String> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?
            .vault
            .is_some())
    }

    pub fn get_password(
        &self,
        app: &AppHandle,
        profile_id: &str,
    ) -> Result<Option<String>, String> {
        match SecretStorageMode::from_config_value(&load_config_file()?.secret_storage_mode) {
            SecretStorageMode::System => self.get_password_from_keyring(profile_id),
            SecretStorageMode::Vault => self.get_password_from_vault(app, profile_id),
            SecretStorageMode::Memory => Ok(None),
            SecretStorageMode::Auto => {
                if self.keyring_available()? {
                    return self.get_password_from_keyring(profile_id);
                }
                self.get_password_from_vault(app, profile_id)
            }
        }
    }

    fn get_password_from_keyring(&self, profile_id: &str) -> Result<Option<String>, String> {
        if !self.keyring_available()? {
            return Ok(None);
        }
        read_keyring_secret(profile_id, SECRET_KIND_PASSWORD)
            .map_err(|err| format!("Failed to read system credential store: {}", err))
    }

    fn get_password_from_vault(
        &self,
        app: &AppHandle,
        profile_id: &str,
    ) -> Result<Option<String>, String> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &guard.vault {
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
        match SecretStorageMode::from_config_value(&load_config_file()?.secret_storage_mode) {
            SecretStorageMode::System => self.save_password_to_keyring(profile_id, password),
            SecretStorageMode::Vault => self.save_password_to_vault(app, profile_id, password),
            SecretStorageMode::Memory => Ok(SecretLocation::Memory),
            SecretStorageMode::Auto => {
                if self.keyring_available()? {
                    return self.save_password_to_keyring(profile_id, password);
                }
                self.save_password_to_vault(app, profile_id, password)
            }
        }
    }

    fn save_password_to_keyring(
        &self,
        profile_id: &str,
        password: &str,
    ) -> Result<SecretLocation, String> {
        if !self.keyring_available()? {
            return Ok(SecretLocation::Memory);
        }
        write_keyring_secret(profile_id, SECRET_KIND_PASSWORD, password)?;
        Ok(SecretLocation::Keyring)
    }

    fn save_password_to_vault(
        &self,
        app: &AppHandle,
        profile_id: &str,
        password: &str,
    ) -> Result<SecretLocation, String> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(runtime) = &guard.vault {
            write_vault_secret(app, runtime, profile_id, SECRET_KIND_PASSWORD, password)?;
            return Ok(SecretLocation::Vault);
        }

        Ok(SecretLocation::Memory)
    }

    pub fn has_password(&self, app: &AppHandle, profile_id: &str) -> Result<bool, String> {
        Ok(self.get_password(app, profile_id)?.is_some())
    }

    pub fn delete_password(&self, app: &AppHandle, profile_id: &str) -> Result<bool, String> {
        let mut deleted = false;
        if self.keyring_available()? {
            deleted |= delete_keyring_secret(profile_id, SECRET_KIND_PASSWORD)?;
        }
        deleted |= delete_vault_secret(app, profile_id, SECRET_KIND_PASSWORD)?;
        Ok(deleted)
    }

    pub fn copy_password(
        &self,
        app: &AppHandle,
        profile_id: &str,
        from: &str,
        to: &str,
    ) -> Result<bool, String> {
        if matches!(
            SecretStorageMode::from_config_value(from),
            SecretStorageMode::Vault
        ) && !self.vault_unlocked()?
        {
            return Err("Unlock the app vault before copying passwords from it.".to_string());
        }
        if matches!(
            SecretStorageMode::from_config_value(to),
            SecretStorageMode::Vault
        ) && !self.vault_unlocked()?
        {
            return Err("Unlock the app vault before copying passwords to it.".to_string());
        }

        let password = match SecretStorageMode::from_config_value(from) {
            SecretStorageMode::System => self.get_password_from_keyring(profile_id)?,
            SecretStorageMode::Vault => self.get_password_from_vault(app, profile_id)?,
            SecretStorageMode::Auto | SecretStorageMode::Memory => None,
        };
        let Some(password) = password else {
            return Ok(false);
        };

        let location = match SecretStorageMode::from_config_value(to) {
            SecretStorageMode::System => self.save_password_to_keyring(profile_id, &password)?,
            SecretStorageMode::Vault => self.save_password_to_vault(app, profile_id, &password)?,
            SecretStorageMode::Auto | SecretStorageMode::Memory => SecretLocation::Memory,
        };

        Ok(!matches!(location, SecretLocation::Memory))
    }
}

impl Drop for SecretStoreRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = &mut self.vault {
            runtime.key.zeroize();
        }
    }
}

fn secret_key_name(profile_id: &str, kind: &str) -> String {
    format!("{}::{}", kind, profile_id)
}

fn read_keyring_secret(profile_id: &str, kind: &str) -> Result<Option<String>, keyring::Error> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err),
    }
}

fn write_keyring_secret(profile_id: &str, kind: &str, value: &str) -> Result<(), String> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .map_err(|e| format!("Failed to open keyring entry: {}", e))?;
    entry
        .set_password(value)
        .map_err(|e| format!("Failed to write keyring secret: {}", e))
}

fn delete_keyring_secret(profile_id: &str, kind: &str) -> Result<bool, String> {
    let account = secret_key_name(profile_id, kind);
    let entry = keyring::Entry::new(SERVICE_NAME, &account)
        .map_err(|e| format!("Failed to open keyring entry: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(err) => Err(format!("Failed to delete keyring secret: {}", err)),
    }
}

fn derive_or_initialize_vault_key(
    app: &AppHandle,
    password: &[u8],
) -> Result<[u8; DERIVED_KEY_LEN], String> {
    let config_path = vault_config_path(app)?;
    let config = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read vault config: {}", e))?;
        let config = serde_json::from_str::<VaultConfigFile>(&content)
            .map_err(|e| format!("Failed to parse vault config: {}", e))?;
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize vault config: {}", e))?;
        fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write vault config: {}", e))?;
        config
    } else {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let config = VaultConfigFile {
            salt_b64: BASE64.encode(salt),
            version: default_vault_config_version(),
            algorithm: default_vault_algorithm(),
            kdf: default_vault_kdf(),
            memory_kib: default_vault_memory_kib(),
            iterations: default_vault_iterations(),
            parallelism: default_vault_parallelism(),
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
        config.memory_kib,
        config.iterations,
        config.parallelism,
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
    runtime: &VaultRuntime,
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
    runtime: &VaultRuntime,
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
    runtime: &VaultRuntime,
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
    runtime: &VaultRuntime,
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

fn delete_vault_secret(
    app: &AppHandle,
    profile_id: &str,
    kind: &str,
) -> Result<bool, String> {
    let path = vault_path(app)?;
    let mut vault = load_vault_file(&path)?;
    let before = vault.secrets.len();
    vault
        .secrets
        .retain(|record| !(record.profile_id == profile_id && record.kind == kind));
    if vault.secrets.len() == before {
        return Ok(false);
    }
    save_vault_file(&path, &vault)?;
    Ok(true)
}

fn verify_or_initialize_vault(app: &AppHandle, runtime: &VaultRuntime) -> Result<(), String> {
    let path = vault_path(app)?;
    let vault = load_vault_file(&path)?;

    if let Some(record) = vault
        .secrets
        .iter()
        .find(|record| record.profile_id == VERIFIER_PROFILE_ID && record.kind == SECRET_KIND_VERIFIER)
    {
        let value = decrypt_secret(runtime, record)?;
        if value == VAULT_VERIFIER_PLAINTEXT {
            return Ok(());
        }
        return Err("Failed to unlock vault. Check the vault password.".to_string());
    }

    if let Some(record) = vault.secrets.first() {
        decrypt_secret(runtime, record).map(|_| ())?;
    }

    write_vault_secret(
        app,
        runtime,
        VERIFIER_PROFILE_ID,
        SECRET_KIND_VERIFIER,
        VAULT_VERIFIER_PLAINTEXT,
    )
}
