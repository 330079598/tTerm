use super::crypto::SecretKey;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub(crate) const SERVICE_NAME: &str = "tterm";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretBackendStatus {
    /// `system`, `password` or `memory`.
    pub storage_mode: String,
    pub keyring_available: bool,
    /// Whether saved passwords can be read and written right now.
    pub unlocked: bool,
    pub has_master_password: bool,
    pub persistence_available: bool,
    /// Passwords from the old app vault are waiting for its password to be
    /// moved into the database.
    pub migration_pending: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultPasswordInput {
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeVaultPasswordInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone)]
pub struct SecretStoreState {
    pub(crate) inner: Arc<Mutex<SecretStoreRuntime>>,
}

#[derive(Debug, Default)]
pub(crate) struct SecretStoreRuntime {
    pub keyring_available: Option<bool>,
    /// The unwrapped data key while saved passwords are unlocked.
    pub data_key: Option<SecretKey>,
    /// A startup problem to show in the status, e.g. a failed migration.
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretLocation {
    Database,
    Memory,
}

/// Where the key that unlocks saved passwords comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecretStorageMode {
    /// The OS credential store; unlocks automatically.
    System,
    /// A master password typed by the user.
    Password,
    /// Nothing is saved.
    Memory,
}

impl SecretStorageMode {
    /// Also accepts the modes from before the database: `auto` and `hybrid`
    /// unlocked without a password like `system`, `vault` needed one.
    pub(crate) fn from_config_value(value: &str) -> Self {
        match value {
            "password" | "vault" => Self::Password,
            "memory" => Self::Memory,
            _ => Self::System,
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "system" => Ok(Self::System),
            "password" => Ok(Self::Password),
            "memory" => Ok(Self::Memory),
            _ => Err("Storage mode must be system, password, or memory.".to_string()),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Password => "password",
            Self::Memory => "memory",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_modes_map_onto_the_new_ones() {
        for (old, new) in [
            ("auto", SecretStorageMode::System),
            ("system", SecretStorageMode::System),
            ("hybrid", SecretStorageMode::System),
            ("vault", SecretStorageMode::Password),
            ("password", SecretStorageMode::Password),
            ("memory", SecretStorageMode::Memory),
            ("", SecretStorageMode::System),
        ] {
            assert_eq!(SecretStorageMode::from_config_value(old), new, "{old}");
        }
        assert!(SecretStorageMode::parse("hybrid").is_err());
    }
}
