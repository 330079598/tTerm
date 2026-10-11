//! The archive format: building, encrypting, decoding and validating a
//! `.tterm-backup` file.

use super::remote;
use super::{
    BackupCrypto, BackupManifest, BackupPayload, BackupSelection, DecodedBundle, BACKUP_AAD,
    ENCRYPTED_PAYLOAD_ENTRY, FORMAT_NAME, FORMAT_VERSION, MANIFEST_ENTRY, MAX_BACKUP_SIZE,
    MAX_PAYLOAD_SIZE, PAYLOAD_ENTRY,
};
use crate::config::AppConfig;
use crate::profiles::SavedProfile;
use crate::session::SessionData;
use crate::sftp::store::SftpDirectoryStore;
use crate::ssh::store::KnownHostStore;
use crate::ssh::SecretStoreState;
use crate::tunnel::TunnelRule;
use aes_gcm::aead::{Aead, KeyInit, Payload as AeadPayload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path};
use tauri::AppHandle;
use zeroize::Zeroize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(super) fn build_archive(
    app: &AppHandle,
    selection: &BackupSelection,
    payload: &BackupPayload,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    build_archive_for_version(
        &app.package_info().version.to_string(),
        selection,
        payload,
        password,
    )
}

pub(super) fn build_archive_for_version(
    app_version: &str,
    selection: &BackupSelection,
    payload: &BackupPayload,
    password: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut payload_bytes = serde_json::to_vec(payload)
        .map_err(|error| format!("Failed to serialize backup payload: {error}"))?;
    let (stored_payload, crypto) = if let Some(password) = password.filter(|p| !p.is_empty()) {
        encrypt_payload(&payload_bytes, password).map(|(bytes, crypto)| (bytes, Some(crypto)))?
    } else {
        (payload_bytes.clone(), None)
    };
    let manifest = BackupManifest {
        format: FORMAT_NAME.to_string(),
        format_version: FORMAT_VERSION,
        app_version: app_version.to_string(),
        created_at: Utc::now().to_rfc3339(),
        platform: std::env::consts::OS.to_string(),
        selection: selection.clone(),
        encrypted: crypto.is_some(),
        payload_sha256: sha256_hex(&stored_payload),
        secret_count: payload.secrets.len(),
        crypto,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Failed to serialize backup manifest: {error}"))?;
    let cursor = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(MANIFEST_ENTRY, options)
        .map_err(|error| format!("Failed to create backup manifest entry: {error}"))?;
    zip.write_all(&manifest_bytes)
        .map_err(|error| format!("Failed to write backup manifest: {error}"))?;
    let entry_name = if manifest.encrypted {
        ENCRYPTED_PAYLOAD_ENTRY
    } else {
        PAYLOAD_ENTRY
    };
    zip.start_file(entry_name, options)
        .map_err(|error| format!("Failed to create backup payload entry: {error}"))?;
    zip.write_all(&stored_payload)
        .map_err(|error| format!("Failed to write backup payload: {error}"))?;
    let archive = zip
        .finish()
        .map_err(|error| format!("Failed to finish backup archive: {error}"))?
        .into_inner();
    payload_bytes.zeroize();
    Ok(archive)
}

/// Decodes with the typed password or, when none was typed and
/// `use_webdav_password` is set, with the saved WebDAV backup password if it
/// opens the backup. Returns whether the saved password was used.
pub(super) fn decode_with_password(
    path: &Path,
    typed: Option<&str>,
    use_webdav_password: bool,
    secret_state: &SecretStoreState,
) -> Result<(DecodedBundle, bool), String> {
    let typed = typed.filter(|password| !password.is_empty());
    if typed.is_none() && use_webdav_password {
        // A locked store or another device's password just asks for it.
        let saved = remote::saved_backup_password(secret_state).ok().flatten();
        if let Some(Ok(bundle)) = saved.map(|saved| decode_archive(path, Some(&saved))) {
            let used = bundle.manifest.encrypted;
            return Ok((bundle, used));
        }
    }
    Ok((decode_archive(path, typed)?, false))
}

pub(super) fn decode_archive(path: &Path, password: Option<&str>) -> Result<DecodedBundle, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect backup '{}': {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_BACKUP_SIZE {
        return Err("Backup file is invalid or exceeds the 128 MiB limit.".to_string());
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Failed to read backup '{}': {error}", path.display()))?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Invalid tTerm backup archive: {error}"))?;
    if archive.len() != 2 {
        return Err("Invalid backup archive layout.".to_string());
    }
    let manifest: BackupManifest = {
        let mut entry = archive
            .by_name(MANIFEST_ENTRY)
            .map_err(|_| "Backup manifest is missing.".to_string())?;
        if entry.size() > 1024 * 1024 {
            return Err("Backup manifest is too large.".to_string());
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| format!("Failed to read backup manifest: {error}"))?;
        serde_json::from_slice(&data)
            .map_err(|error| format!("Invalid backup manifest: {error}"))?
    };
    validate_manifest(&manifest)?;
    let entry_name = if manifest.encrypted {
        ENCRYPTED_PAYLOAD_ENTRY
    } else {
        PAYLOAD_ENTRY
    };
    let mut stored = {
        let mut entry = archive
            .by_name(entry_name)
            .map_err(|_| "Backup payload is missing.".to_string())?;
        if entry.size() > MAX_PAYLOAD_SIZE {
            return Err("Backup payload exceeds the 64 MiB limit.".to_string());
        }
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|error| format!("Failed to read backup payload: {error}"))?;
        data
    };
    if sha256_hex(&stored) != manifest.payload_sha256 {
        return Err("Backup integrity check failed.".to_string());
    }
    if manifest.encrypted && password.filter(|p| !p.is_empty()).is_none() {
        return Ok(DecodedBundle {
            manifest,
            payload: None,
        });
    }
    let mut plain = if manifest.encrypted {
        decrypt_payload(
            &stored,
            password.unwrap_or_default(),
            manifest
                .crypto
                .as_ref()
                .ok_or_else(|| "Encrypted backup has no crypto metadata.".to_string())?,
        )?
    } else {
        stored.clone()
    };
    let payload = serde_json::from_slice::<BackupPayload>(&plain)
        .map_err(|error| format!("Invalid backup payload: {error}"))?;
    plain.zeroize();
    stored.zeroize();
    validate_payload(&payload)?;
    Ok(DecodedBundle {
        manifest,
        payload: Some(payload),
    })
}

fn validate_manifest(manifest: &BackupManifest) -> Result<(), String> {
    if manifest.format != FORMAT_NAME {
        return Err("The selected file is not a tTerm backup.".to_string());
    }
    if manifest.format_version == 0 || manifest.format_version > FORMAT_VERSION {
        return Err(format!(
            "Backup format version {} is not supported by this version of tTerm.",
            manifest.format_version
        ));
    }
    if manifest.encrypted != manifest.crypto.is_some() {
        return Err("Backup encryption metadata is inconsistent.".to_string());
    }
    Ok(())
}

pub(super) fn validate_payload(payload: &BackupPayload) -> Result<(), String> {
    if let Some(value) = payload.config.clone() {
        serde_json::from_value::<AppConfig>(value)
            .map_err(|error| format!("Invalid settings data in backup: {error}"))?;
    }
    if let Some(value) = payload.profiles.clone() {
        serde_json::from_value::<Vec<SavedProfile>>(value)
            .map_err(|error| format!("Invalid profile data in backup: {error}"))?;
    }
    if let Some(value) = payload.tunnels.clone() {
        serde_json::from_value::<Vec<TunnelRule>>(value)
            .map_err(|error| format!("Invalid tunnel data in backup: {error}"))?;
    }
    if let Some(value) = payload.session.clone() {
        serde_json::from_value::<SessionData>(value)
            .map_err(|error| format!("Invalid session data in backup: {error}"))?;
    }
    if let Some(value) = payload.known_hosts.clone() {
        serde_json::from_value::<KnownHostStore>(value)
            .map_err(|error| format!("Invalid known-host data in backup: {error}"))?;
    }
    if let Some(value) = payload.sftp_directories.clone() {
        serde_json::from_value::<SftpDirectoryStore>(value)
            .map_err(|error| format!("Invalid SFTP data in backup: {error}"))?;
    }
    let mut secret_keys = HashSet::new();
    for secret in &payload.secrets {
        if secret.key.trim().is_empty() || !secret_keys.insert(secret.key.as_str()) {
            return Err("Backup contains an invalid or duplicate password key.".to_string());
        }
    }
    let mut total_log_bytes = 0usize;
    let mut log_paths = HashSet::new();
    if payload.logs.len() > 5_000 {
        return Err("Backup contains too many terminal log files.".to_string());
    }
    for log in &payload.logs {
        validate_relative_path(&log.relative_path)?;
        if !log_paths.insert(log.relative_path.as_str()) {
            return Err("Backup contains a duplicate terminal log path.".to_string());
        }
        let decoded_len = BASE64
            .decode(&log.data_b64)
            .map_err(|_| "Backup contains invalid terminal log data.".to_string())?
            .len();
        total_log_bytes = total_log_bytes.saturating_add(decoded_len);
        if total_log_bytes > 48 * 1024 * 1024 {
            return Err("Terminal logs in backup exceed the 48 MiB limit.".to_string());
        }
    }
    Ok(())
}

pub(super) fn encrypt_payload(
    plaintext: &[u8],
    password: &str,
) -> Result<(Vec<u8>, BackupCrypto), String> {
    let memory_kib = 65_536;
    let iterations = 3;
    let parallelism = 1;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut salt);
    rand::rng().fill_bytes(&mut nonce);
    let mut key_bytes = derive_key(password, &salt, memory_kib, iterations, parallelism)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("backup keys are 32 bytes");
    let ciphertext = cipher
        .encrypt(
            &Nonce::from(nonce),
            AeadPayload {
                msg: plaintext,
                aad: BACKUP_AAD,
            },
        )
        .map_err(|_| "Failed to encrypt backup payload.".to_string())?;
    key_bytes.zeroize();
    Ok((
        ciphertext,
        BackupCrypto {
            algorithm: "AES-256-GCM".to_string(),
            kdf: "Argon2id".to_string(),
            salt_b64: BASE64.encode(salt),
            nonce_b64: BASE64.encode(nonce),
            memory_kib,
            iterations,
            parallelism,
        },
    ))
}

pub(super) fn decrypt_payload(
    ciphertext: &[u8],
    password: &str,
    crypto: &BackupCrypto,
) -> Result<Vec<u8>, String> {
    if crypto.algorithm != "AES-256-GCM" || crypto.kdf != "Argon2id" {
        return Err("Backup encryption algorithm is not supported.".to_string());
    }
    let salt = BASE64
        .decode(&crypto.salt_b64)
        .map_err(|_| "Backup salt is invalid.".to_string())?;
    let nonce = BASE64
        .decode(&crypto.nonce_b64)
        .map_err(|_| "Backup nonce is invalid.".to_string())?;
    let nonce = Nonce::try_from(nonce.as_slice())
        .ok()
        .filter(|_| salt.len() == 16)
        .ok_or_else(|| "Backup encryption metadata is invalid.".to_string())?;
    let mut key_bytes = derive_key(
        password,
        &salt,
        crypto.memory_kib,
        crypto.iterations,
        crypto.parallelism,
    )?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("backup keys are 32 bytes");
    let result = cipher
        .decrypt(
            &nonce,
            AeadPayload {
                msg: ciphertext,
                aad: BACKUP_AAD,
            },
        )
        .map_err(|_| "Unable to decrypt backup. Check the backup password.".to_string());
    key_bytes.zeroize();
    result
}

pub(crate) fn derive_key(
    password: &str,
    salt: &[u8],
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<[u8; 32], String> {
    if memory_kib > 262_144 || iterations > 10 || parallelism > 8 {
        return Err("Backup KDF parameters exceed supported limits.".to_string());
    }
    let params = Params::new(memory_kib, iterations, parallelism, Some(32))
        .map_err(|error| format!("Invalid backup KDF parameters: {error}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| format!("Failed to derive backup key: {error}"))?;
    Ok(key)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn validate_relative_path(value: &str) -> Result<(), String> {
    if value.is_empty() || value.contains('\\') {
        return Err("Backup contains an invalid relative path.".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Backup contains an unsafe relative path.".to_string());
    }
    Ok(())
}
