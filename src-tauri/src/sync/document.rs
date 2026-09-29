//! The file every device syncs through: `tterm-sync.enc` in the WebDAV
//! folder. A small JSON envelope carries the key-derivation settings and the
//! AES-256-GCM ciphertext of the gzip-compressed collections, encrypted with
//! a key derived from the backup password.

use super::merge::Collection;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use zeroize::Zeroizing;

pub(crate) const SYNC_FILE_NAME: &str = "tterm-sync.enc";
/// Larger than any real document; bounds what a broken server can make us read.
pub(crate) const MAX_DOCUMENT_SIZE: u64 = 64 * 1024 * 1024;
const FORMAT: &str = "tterm-sync";
const FORMAT_VERSION: u32 = 1;
const AAD: &[u8] = b"tterm-sync-v1";
const MAX_PLAINTEXT_SIZE: u64 = 256 * 1024 * 1024;

/// Collections by category name. A category no device has synced yet is absent.
pub(crate) type Collections = BTreeMap<String, Collection>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KdfSpec {
    salt_b64: String,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl KdfSpec {
    fn generate() -> Self {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        Self {
            salt_b64: BASE64.encode(salt),
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    format: String,
    format_version: u32,
    /// Informational: who wrote the file and when.
    updated_at: i64,
    updated_by: String,
    app_version: String,
    kdf: KdfSpec,
    nonce_b64: String,
    ciphertext_b64: String,
}

/// What is encrypted. The revision history is inside, so the server cannot
/// forge a lineage that would make a device treat missing records as deleted.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Content {
    /// Revisions this file descends from, oldest first; the last is its own.
    history: Vec<String>,
    collections: Collections,
}

pub(crate) struct Decoded {
    pub collections: Collections,
    pub history: Vec<String>,
    /// Reused when writing the next version, so the cached key still applies.
    pub kdf: KdfSpec,
}

pub(crate) struct Encoded {
    pub bytes: Vec<u8>,
    pub kdf: KdfSpec,
    pub history: Vec<String>,
}

/// How many ancestors a file remembers. A device whose last sync is older
/// than that merges like a first sync, which deletes nothing.
const HISTORY_LEN: usize = 100;

/// The history of a new revision made on top of `parent` (the history of the
/// file it was merged with, if any).
fn next_history(parent: &[String]) -> Vec<String> {
    let mut history: Vec<String> = parent
        .iter()
        .skip(parent.len().saturating_sub(HISTORY_LEN - 1))
        .cloned()
        .collect();
    history.push(uuid::Uuid::new_v4().to_string());
    history
}

/// The last derived key. Deriving costs about a second, and the salt only
/// changes when a new document is started.
static KEY_CACHE: Mutex<Option<CachedKey>> = Mutex::new(None);

struct CachedKey {
    kdf: KdfSpec,
    password_digest: [u8; 32],
    key: Zeroizing<[u8; 32]>,
}

fn key_for(password: &str, kdf: &KdfSpec) -> Result<Zeroizing<[u8; 32]>, String> {
    let digest: [u8; 32] = Sha256::digest(password.as_bytes()).into();
    if let Ok(cache) = KEY_CACHE.lock() {
        if let Some(cached) = cache.as_ref() {
            if cached.kdf == *kdf && cached.password_digest == digest {
                return Ok(cached.key.clone());
            }
        }
    }
    let salt = BASE64
        .decode(&kdf.salt_b64)
        .map_err(|_| "Sync file key salt is invalid.".to_string())?;
    let key = Zeroizing::new(crate::backup::derive_key(
        password,
        &salt,
        kdf.memory_kib,
        kdf.iterations,
        kdf.parallelism,
    )?);
    if let Ok(mut cache) = KEY_CACHE.lock() {
        *cache = Some(CachedKey {
            kdf: kdf.clone(),
            password_digest: digest,
            key: key.clone(),
        });
    }
    Ok(key)
}

pub(crate) fn encode(
    collections: &Collections,
    parent_history: &[String],
    password: &str,
    kdf: Option<&KdfSpec>,
    updated_by: &str,
    app_version: &str,
) -> Result<Encoded, String> {
    let kdf = kdf.cloned().unwrap_or_else(KdfSpec::generate);
    let history = next_history(parent_history);
    let content = Content {
        history,
        collections: collections.clone(),
    };
    let json = Zeroizing::new(
        serde_json::to_vec(&content)
            .map_err(|error| format!("Failed to serialize sync data: {error}"))?,
    );
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&json)
        .map_err(|error| format!("Failed to compress sync data: {error}"))?;
    let compressed = Zeroizing::new(
        encoder
            .finish()
            .map_err(|error| format!("Failed to compress sync data: {error}"))?,
    );
    let key = key_for(password, &kdf)?;
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key[..]))
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &compressed,
                aad: AAD,
            },
        )
        .map_err(|_| "Failed to encrypt sync data.".to_string())?;
    let envelope = Envelope {
        format: FORMAT.to_string(),
        format_version: FORMAT_VERSION,
        updated_at: chrono::Utc::now().timestamp_millis(),
        updated_by: updated_by.to_string(),
        app_version: app_version.to_string(),
        kdf: kdf.clone(),
        nonce_b64: BASE64.encode(nonce),
        ciphertext_b64: BASE64.encode(ciphertext),
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|error| format!("Failed to write sync file: {error}"))?;
    Ok(Encoded {
        bytes,
        kdf,
        history: content.history,
    })
}

pub(crate) fn decode(bytes: &[u8], password: &str) -> Result<Decoded, String> {
    let envelope: Envelope = serde_json::from_slice(bytes)
        .map_err(|_| "The remote sync file is not a tTerm sync file.".to_string())?;
    if envelope.format != FORMAT {
        return Err("The remote sync file is not a tTerm sync file.".to_string());
    }
    if envelope.format_version == 0 || envelope.format_version > FORMAT_VERSION {
        return Err(format!(
            "The remote sync file was written by a newer tTerm ({}). Update tTerm on this device.",
            envelope.app_version
        ));
    }
    let nonce = BASE64
        .decode(&envelope.nonce_b64)
        .ok()
        .filter(|nonce| nonce.len() == 12)
        .ok_or_else(|| "The remote sync file is damaged.".to_string())?;
    let ciphertext = BASE64
        .decode(&envelope.ciphertext_b64)
        .map_err(|_| "The remote sync file is damaged.".to_string())?;
    let key = key_for(password, &envelope.kdf)?;
    let compressed = Zeroizing::new(
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key[..]))
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| {
                "Could not decrypt the remote sync file. Another device may have changed the backup password; enter the current one in the WebDAV settings."
                    .to_string()
            })?,
    );
    let mut json = Zeroizing::new(Vec::new());
    GzDecoder::new(&compressed[..])
        .take(MAX_PLAINTEXT_SIZE + 1)
        .read_to_end(&mut json)
        .map_err(|_| "The remote sync file is damaged.".to_string())?;
    if json.len() as u64 > MAX_PLAINTEXT_SIZE {
        return Err("The remote sync file is too large.".to_string());
    }
    let content: Content = serde_json::from_slice(&json)
        .map_err(|error| format!("The remote sync file is damaged: {error}"))?;
    if content.history.is_empty() {
        return Err("The remote sync file is damaged: it has no revision.".to_string());
    }
    Ok(Decoded {
        collections: content.collections,
        history: content.history,
        kdf: envelope.kdf,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::merge::SyncRecord;
    use serde_json::json;

    fn sample() -> Collections {
        let mut profiles = Collection::new();
        profiles.insert(
            "p1".to_string(),
            SyncRecord::new(
                json!({"host": "example.com", "password": "hunter2"}),
                Some(5),
            ),
        );
        Collections::from([("profiles".to_string(), profiles)])
    }

    #[test]
    fn round_trips_and_hides_the_content() {
        let encoded = encode(&sample(), &[], "sync-password", None, "mac", "1.0.0").unwrap();
        assert!(!String::from_utf8_lossy(&encoded.bytes).contains("hunter2"));
        let decoded = decode(&encoded.bytes, "sync-password").unwrap();
        assert_eq!(decoded.collections, sample());
        assert_eq!(decoded.kdf, encoded.kdf);
        assert_eq!(decoded.history, encoded.history);
        assert_eq!(decoded.history.len(), 1);

        // The next revision reuses the salt, not the nonce, and extends the history.
        let again = encode(
            &sample(),
            &decoded.history,
            "sync-password",
            Some(&decoded.kdf),
            "mac",
            "1.0.0",
        )
        .unwrap();
        let next = decode(&again.bytes, "sync-password").unwrap();
        assert_eq!(next.kdf, decoded.kdf);
        assert_ne!(again.bytes, encoded.bytes);
        assert_eq!(next.history[0], decoded.history[0]);
        assert_ne!(next.history.last(), decoded.history.last());
    }

    #[test]
    fn history_is_bounded() {
        let parent: Vec<String> = (0..150).map(|i| i.to_string()).collect();
        let history = next_history(&parent);
        assert_eq!(history.len(), HISTORY_LEN);
        assert_eq!(history[0], "51");
        assert_eq!(history[HISTORY_LEN - 2], "149");
    }

    #[test]
    fn rejects_wrong_passwords_and_foreign_files() {
        let bytes = encode(&sample(), &[], "sync-password", None, "mac", "1.0.0")
            .unwrap()
            .bytes;
        let error = decode(&bytes, "other-password").err().unwrap();
        assert!(error.contains("Could not decrypt"), "{error}");
        assert!(decode(b"PK\x03\x04", "x").is_err());
        let mut envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        envelope["formatVersion"] = json!(99);
        envelope["appVersion"] = json!("9.9.9");
        let error = decode(&serde_json::to_vec(&envelope).unwrap(), "sync-password")
            .err()
            .unwrap();
        assert!(error.contains("9.9.9"), "{error}");
    }
}
