use super::types::{
    default_vault_algorithm, default_vault_config_version, default_vault_iterations,
    default_vault_kdf, default_vault_memory_kib, default_vault_parallelism, VaultConfigFile,
    VaultFile, VaultRuntime, VaultSecretRecord, DERIVED_KEY_LEN, NONCE_LEN, SALT_LEN,
    SECRET_KIND_VERIFIER, VAULT_CONFIG_FILE_NAME, VAULT_FILE_NAME, VAULT_VERIFIER_PLAINTEXT,
    VERIFIER_PROFILE_ID,
};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub(crate) fn derive_or_initialize_vault_key(
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
        crate::config::atomic_write_private(&config_path, content)?;
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
        crate::config::atomic_write_private(&config_path, content)?;
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
    restrict_secret_directory(&dir)?;
    Ok(dir)
}

#[cfg(unix)]
fn restrict_secret_directory(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to restrict secrets directory permissions: {}", e))
}

#[cfg(not(unix))]
fn restrict_secret_directory(_dir: &Path) -> Result<(), String> {
    Ok(())
}

pub(crate) fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_secret_dir(app)?.join(VAULT_FILE_NAME))
}

pub(crate) fn vault_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_secret_dir(app)?.join(VAULT_CONFIG_FILE_NAME))
}

pub(crate) fn load_vault_file(path: &Path) -> Result<VaultFile, String> {
    if !path.exists() {
        return Ok(VaultFile::default());
    }
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read vault: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse vault: {}", e))
}

pub(crate) fn save_vault_file(path: &Path, vault: &VaultFile) -> Result<(), String> {
    let content = serde_json::to_string_pretty(vault)
        .map_err(|e| format!("Failed to serialize vault: {}", e))?;
    crate::config::atomic_write_private(path, content)
}

pub(crate) fn encrypt_secret(
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

pub(crate) fn decrypt_secret(
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

pub(crate) fn read_vault_secret(
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

pub(crate) fn write_vault_secret(
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

pub(crate) fn delete_vault_secret(
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

pub(crate) fn verify_or_initialize_vault(
    app: &AppHandle,
    runtime: &VaultRuntime,
) -> Result<(), String> {
    let path = vault_path(app)?;
    let vault = load_vault_file(&path)?;

    if let Some(record) = vault.secrets.iter().find(|record| {
        record.profile_id == VERIFIER_PROFILE_ID && record.kind == SECRET_KIND_VERIFIER
    }) {
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
