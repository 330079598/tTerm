mod keyring_backend;
mod types;
mod vault;

use crate::config::{load_config_file, save_config_file};
use keyring_backend::{delete_keyring_secret, read_keyring_secret, write_keyring_secret};
use rand::RngCore;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use types::{
    DERIVED_KEY_LEN, PROBE_ACCOUNT, KEYRING_PROBE_SECRET,
    SecretStorageMode, SecretStoreRuntime,
    VAULT_MASTER_ACCOUNT, VaultRuntime,
};
use vault::{
    decrypt_secret, derive_or_initialize_vault_key, delete_vault_secret, encrypt_secret,
    load_vault_file, read_vault_secret, save_vault_file, vault_config_path, vault_path,
    verify_or_initialize_vault, write_vault_secret,
};
use zeroize::Zeroize;

pub use types::{
    ChangeVaultPasswordInput, SecretBackendStatus, SecretLocation, SecretStoreState,
    VaultPasswordInput,
};

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
            SecretStorageMode::Vault if config.secret_vault_enabled && vault_unlocked => "vault",
            SecretStorageMode::Hybrid if config.secret_vault_enabled && vault_unlocked => "vault",
            SecretStorageMode::Auto if keyring_available => "system",
            SecretStorageMode::Auto if config.secret_vault_enabled && vault_unlocked => "vault",
            _ => "memory",
        };
        let persistence_available = match mode {
            SecretStorageMode::System => keyring_available,
            SecretStorageMode::Vault => config.secret_vault_enabled && vault_unlocked,
            SecretStorageMode::Hybrid => config.secret_vault_enabled && vault_unlocked,
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
            SecretStorageMode::Hybrid if !config.secret_vault_enabled => Some(
                "Hybrid mode requires the app vault. Set a vault password to enable it."
                    .to_string(),
            ),
            SecretStorageMode::Hybrid if !vault_unlocked => Some(
                "Hybrid mode requires an unlocked vault. Enter the vault password.".to_string(),
            ),
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
        let entry = match keyring::Entry::new(
            types::SERVICE_NAME,
            PROBE_ACCOUNT,
        ) {
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

        let mode = SecretStorageMode::from_config_value(&config.secret_storage_mode);
        if mode == SecretStorageMode::Hybrid {
            if !self.keyring_available()? {
                return Err(
                    "Hybrid mode requires the system credential store to save the vault master password."
                        .to_string(),
                );
            }
            write_keyring_secret(VAULT_MASTER_ACCOUNT, "master", &input.password)?;
        }

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        guard.vault = Some(runtime);
        drop(guard);

        self.get_status()
    }

    pub fn change_vault_password(
        &self,
        app: &AppHandle,
        input: ChangeVaultPasswordInput,
    ) -> Result<SecretBackendStatus, String> {
        if input.current_password.is_empty() || input.new_password.is_empty() {
            return Err("Passwords cannot be empty".to_string());
        }

        let mut stored_key = {
            let guard = self
                .inner
                .lock()
                .map_err(|_| "Secret store state is poisoned".to_string())?;
            match &guard.vault {
                Some(rt) => rt.key,
                None => return Err("Vault must be unlocked before changing the password.".to_string()),
            }
        };
        let old_runtime = VaultRuntime { key: stored_key };

        let mut derived_key = derive_or_initialize_vault_key(app, input.current_password.as_bytes())?;
        if derived_key != old_runtime.key {
            return Err("Current password is incorrect.".to_string());
        }
        derived_key.zeroize();

        let path = vault_path(app)?;
        let vault = load_vault_file(&path)?;

        let mut decrypted: Vec<(String, String, String, i64)> = Vec::new();
        for record in &vault.secrets {
            let plaintext = decrypt_secret(&old_runtime, record)?;
            decrypted.push((
                record.profile_id.clone(),
                record.kind.clone(),
                plaintext,
                record.updated_at,
            ));
        }

        let mut salt = [0u8; DERIVED_KEY_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        let new_config = types::VaultConfigFile {
            salt_b64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                salt,
            ),
            version: types::default_vault_config_version(),
            algorithm: types::default_vault_algorithm(),
            kdf: types::default_vault_kdf(),
            memory_kib: types::default_vault_memory_kib(),
            iterations: types::default_vault_iterations(),
            parallelism: types::default_vault_parallelism(),
        };

        let config_path = vault_config_path(app)?;
        let content = serde_json::to_string_pretty(&new_config)
            .map_err(|e| format!("Failed to serialize vault config: {}", e))?;
        std::fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write vault config: {}", e))?;

        let salt_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            new_config.salt_b64.as_bytes(),
        )
        .map_err(|e| format!("Failed to decode vault salt: {}", e))?;
        let params = argon2::Params::new(
            new_config.memory_kib,
            new_config.iterations,
            new_config.parallelism,
            Some(DERIVED_KEY_LEN),
        )
        .map_err(|e| format!("Failed to build Argon2 params: {}", e))?;
        let argon2 = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        let mut new_key = [0u8; DERIVED_KEY_LEN];
        argon2
            .hash_password_into(input.new_password.as_bytes(), &salt_bytes, &mut new_key)
            .map_err(|e| format!("Failed to derive vault key: {}", e))?;

        let new_runtime = VaultRuntime { key: new_key };

        let mut new_records: Vec<types::VaultSecretRecord> = Vec::new();
        for (profile_id, kind, plaintext, updated_at) in &decrypted {
            let (nonce_b64, ciphertext_b64) = encrypt_secret(&new_runtime, plaintext)?;
            new_records.push(types::VaultSecretRecord {
                profile_id: profile_id.clone(),
                kind: kind.clone(),
                nonce_b64,
                ciphertext_b64,
                updated_at: *updated_at,
            });
        }

        let new_vault = types::VaultFile {
            secrets: new_records,
        };
        save_vault_file(&path, &new_vault)?;

        stored_key.zeroize();

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        if let Some(old) = &mut guard.vault {
            old.key.zeroize();
        }
        guard.vault = Some(new_runtime);
        drop(guard);

        let config = load_config_file()?;
        let mode = SecretStorageMode::from_config_value(&config.secret_storage_mode);
        if mode == SecretStorageMode::Hybrid {
            write_keyring_secret(VAULT_MASTER_ACCOUNT, "master", &input.new_password)?;
        }

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
        if !enabled {
            self.delete_vault_master_password_from_keyring()?;
        }

        config.secret_vault_enabled = enabled;
        if enabled && config.secret_storage_mode == "memory" {
            config.secret_storage_mode = "auto".to_string();
        }
        if !enabled && config.secret_storage_mode == "vault" {
            config.secret_storage_mode = "auto".to_string();
        }
        if !enabled && config.secret_storage_mode == "hybrid" {
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
        let previous_mode = SecretStorageMode::from_config_value(&config.secret_storage_mode);
        if previous_mode == SecretStorageMode::Hybrid && mode != SecretStorageMode::Hybrid {
            self.delete_vault_master_password_from_keyring()?;
        }

        config.secret_storage_mode = mode.as_str().to_string();
        config.secret_vault_enabled = config.secret_vault_enabled
            || mode == SecretStorageMode::Vault
            || mode == SecretStorageMode::Hybrid;
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
            SecretStorageMode::Vault | SecretStorageMode::Hybrid => {
                self.get_password_from_vault(app, profile_id)
            }
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
        read_keyring_secret(profile_id, types::SECRET_KIND_PASSWORD)
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
            return read_vault_secret(app, runtime, profile_id, types::SECRET_KIND_PASSWORD);
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
            SecretStorageMode::Vault | SecretStorageMode::Hybrid => {
                self.save_password_to_vault(app, profile_id, password)
            }
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
        write_keyring_secret(profile_id, types::SECRET_KIND_PASSWORD, password)?;
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
            write_vault_secret(app, runtime, profile_id, types::SECRET_KIND_PASSWORD, password)?;
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
            deleted |= delete_keyring_secret(profile_id, types::SECRET_KIND_PASSWORD)?;
        }
        deleted |= delete_vault_secret(app, profile_id, types::SECRET_KIND_PASSWORD)?;
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
            SecretStorageMode::Vault | SecretStorageMode::Hybrid
        ) && !self.vault_unlocked()?
        {
            return Err("Unlock the app vault before copying passwords from it.".to_string());
        }
        if matches!(
            SecretStorageMode::from_config_value(to),
            SecretStorageMode::Vault | SecretStorageMode::Hybrid
        ) && !self.vault_unlocked()?
        {
            return Err("Unlock the app vault before copying passwords to it.".to_string());
        }

        let password = match SecretStorageMode::from_config_value(from) {
            SecretStorageMode::System => self.get_password_from_keyring(profile_id)?,
            SecretStorageMode::Vault | SecretStorageMode::Hybrid => {
                self.get_password_from_vault(app, profile_id)?
            }
            SecretStorageMode::Auto | SecretStorageMode::Memory => None,
        };
        let Some(password) = password else {
            return Ok(false);
        };

        let location = match SecretStorageMode::from_config_value(to) {
            SecretStorageMode::System => self.save_password_to_keyring(profile_id, &password)?,
            SecretStorageMode::Vault | SecretStorageMode::Hybrid => {
                self.save_password_to_vault(app, profile_id, &password)?
            }
            SecretStorageMode::Auto | SecretStorageMode::Memory => SecretLocation::Memory,
        };

        Ok(!matches!(location, SecretLocation::Memory))
    }

    pub fn try_auto_unlock_hybrid(&self, app: &AppHandle) -> Result<bool, String> {
        if !self.keyring_available()? {
            return Ok(false);
        }

        let config = load_config_file()?;
        if !config.secret_vault_enabled {
            return Ok(false);
        }

        let Some(password) = read_keyring_secret(VAULT_MASTER_ACCOUNT, "master")
            .map_err(|e| format!("Failed to read keyring: {}", e))?
        else {
            return Ok(false);
        };

        let key = derive_or_initialize_vault_key(app, password.as_bytes())?;
        let runtime = VaultRuntime { key };
        verify_or_initialize_vault(app, &runtime)?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Secret store state is poisoned".to_string())?;
        guard.vault = Some(runtime);

        Ok(true)
    }

    fn delete_vault_master_password_from_keyring(&self) -> Result<bool, String> {
        if !self.keyring_available()? {
            return Ok(false);
        }
        delete_keyring_secret(VAULT_MASTER_ACCOUNT, "master")
    }
}

impl Drop for SecretStoreRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = &mut self.vault {
            runtime.key.zeroize();
        }
    }
}
