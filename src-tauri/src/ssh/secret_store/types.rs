use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

pub(crate) const SERVICE_NAME: &str = "tterm";
pub(crate) const VAULT_FILE_NAME: &str = "secret_vault.json";
pub(crate) const VAULT_CONFIG_FILE_NAME: &str = "secret_vault_config.json";
pub(crate) const SECRET_KIND_PASSWORD: &str = "password";
pub(crate) const SECRET_KIND_VERIFIER: &str = "verifier";
pub(crate) const VERIFIER_PROFILE_ID: &str = "__vault_verifier__";
pub(crate) const VAULT_MASTER_ACCOUNT: &str = "__vault_master__";
pub(crate) const VAULT_VERIFIER_PLAINTEXT: &str = "tterm-vault-verifier-v1";
pub(crate) const PROBE_ACCOUNT: &str = "__probe__";
pub(crate) const KEYRING_PROBE_SECRET: &str = "tterm-keyring-probe";
pub(crate) const SALT_LEN: usize = 16;
pub(crate) const NONCE_LEN: usize = 12;
pub(crate) const DERIVED_KEY_LEN: usize = 32;
pub(crate) const PBKDF_ITERATIONS: u32 = 3;
pub(crate) const PBKDF_MEMORY_KIB: u32 = 19_456;
pub(crate) const PBKDF_PARALLELISM: u32 = 1;

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeVaultPasswordInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone)]
pub struct SecretStoreState {
    pub(crate) inner: std::sync::Arc<std::sync::Mutex<SecretStoreRuntime>>,
}

#[derive(Debug)]
pub(crate) struct SecretStoreRuntime {
    pub cached_keyring_available: Option<bool>,
    pub vault: Option<VaultRuntime>,
    pub vault_file: Option<Arc<RwLock<VaultFile>>>,
    pub vault_gate: Arc<RwLock<()>>,
}

impl Default for SecretStoreRuntime {
    fn default() -> Self {
        Self {
            cached_keyring_available: None,
            vault: None,
            vault_file: None,
            vault_gate: Arc::new(RwLock::new(())),
        }
    }
}

#[derive(Debug)]
pub(crate) struct VaultRuntime {
    pub key: [u8; DERIVED_KEY_LEN],
}

#[derive(Debug, Clone)]
pub enum SecretLocation {
    Keyring,
    Vault,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretStorageMode {
    Auto,
    System,
    Vault,
    Hybrid,
    Memory,
}

impl SecretStorageMode {
    pub(crate) fn from_config_value(value: &str) -> Self {
        match value {
            "system" => Self::System,
            "vault" => Self::Vault,
            "hybrid" => Self::Hybrid,
            "memory" => Self::Memory,
            _ => Self::Auto,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::System => "system",
            Self::Vault => "vault",
            Self::Hybrid => "hybrid",
            Self::Memory => "memory",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct VaultConfigFile {
    #[serde(default)]
    pub salt_b64: String,
    #[serde(default = "default_vault_config_version")]
    pub version: u16,
    #[serde(default = "default_vault_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_vault_kdf")]
    pub kdf: String,
    #[serde(default = "default_vault_memory_kib")]
    pub memory_kib: u32,
    #[serde(default = "default_vault_iterations")]
    pub iterations: u32,
    #[serde(default = "default_vault_parallelism")]
    pub parallelism: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct VaultFile {
    #[serde(default)]
    pub secrets: Vec<VaultSecretRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct VaultSecretRecord {
    pub profile_id: String,
    pub kind: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String,
    pub updated_at: i64,
}

pub(crate) fn default_vault_config_version() -> u16 {
    1
}

pub(crate) fn default_vault_algorithm() -> String {
    "AES-256-GCM".to_string()
}

pub(crate) fn default_vault_kdf() -> String {
    "Argon2id-v1.3".to_string()
}

pub(crate) fn default_vault_memory_kib() -> u32 {
    PBKDF_MEMORY_KIB
}

pub(crate) fn default_vault_iterations() -> u32 {
    PBKDF_ITERATIONS
}

pub(crate) fn default_vault_parallelism() -> u32 {
    PBKDF_PARALLELISM
}
