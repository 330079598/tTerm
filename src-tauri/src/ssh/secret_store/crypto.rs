//! Envelope encryption for saved secrets: one random data key encrypts every
//! secret, and the data key itself is stored only wrapped by a key-encryption
//! key (from the OS credential store or derived from the master password).

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub(crate) const KEY_LEN: usize = 32;
pub(crate) const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;
const SECRET_AAD_PREFIX: &str = "tterm-secret-v1:";
const WRAP_AAD_PREFIX: &str = "tterm-data-key-v1:";

/// A 256-bit AES key that is wiped from memory when dropped.
#[derive(Clone)]
pub(crate) struct SecretKey(Zeroizing<[u8; KEY_LEN]>);

impl SecretKey {
    pub(crate) fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        rand::thread_rng().fill_bytes(&mut bytes[..]);
        Self(bytes)
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self, String> {
        let bytes: [u8; KEY_LEN] = bytes
            .try_into()
            .map_err(|_| "Stored key has the wrong length".to_string())?;
        Ok(Self(Zeroizing::new(bytes)))
    }

    pub(crate) fn to_base64(&self) -> Zeroizing<String> {
        Zeroizing::new(BASE64.encode(&self.0[..]))
    }

    pub(crate) fn from_base64(value: &str) -> Result<Self, String> {
        let mut bytes = BASE64
            .decode(value.trim().as_bytes())
            .map_err(|_| "Stored key is not valid base64".to_string())?;
        let key = Self::from_slice(&bytes);
        bytes.zeroize();
        key
    }

    pub(crate) fn expose(&self) -> &[u8] {
        &self.0[..]
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.0[..]))
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(..)")
    }
}

pub(crate) struct Sealed {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

fn seal(key: &SecretKey, aad: &[u8], plaintext: &[u8]) -> Result<Sealed, String> {
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = key
        .cipher()
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| "Failed to encrypt secret".to_string())?;
    Ok(Sealed {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

fn open(
    key: &SecretKey,
    aad: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Option<Zeroizing<Vec<u8>>> {
    if nonce.len() != NONCE_LEN {
        return None;
    }
    key.cipher()
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .ok()
        .map(Zeroizing::new)
}

/// Encrypts a secret. The lookup key is bound as associated data, so a
/// ciphertext copied onto another row does not decrypt.
pub(crate) fn encrypt_secret(
    data_key: &SecretKey,
    key: &str,
    plaintext: &str,
) -> Result<Sealed, String> {
    seal(data_key, secret_aad(key).as_bytes(), plaintext.as_bytes())
}

pub(crate) fn decrypt_secret(
    data_key: &SecretKey,
    key: &str,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<String>, String> {
    let plaintext = open(data_key, secret_aad(key).as_bytes(), nonce, ciphertext)
        .ok_or_else(|| format!("Saved password for '{key}' could not be decrypted"))?;
    String::from_utf8(plaintext.to_vec())
        .map(Zeroizing::new)
        .map_err(|_| format!("Saved password for '{key}' is not valid UTF-8"))
}

pub(crate) fn wrap_data_key(
    wrapping_key: &SecretKey,
    kind: &str,
    data_key: &SecretKey,
) -> Result<Sealed, String> {
    seal(wrapping_key, wrap_aad(kind).as_bytes(), data_key.expose())
}

/// `None` means the wrapping key is wrong (e.g. a mistyped master password).
pub(crate) fn unwrap_data_key(
    wrapping_key: &SecretKey,
    kind: &str,
    nonce: &[u8],
    wrapped: &[u8],
) -> Option<SecretKey> {
    let bytes = open(wrapping_key, wrap_aad(kind).as_bytes(), nonce, wrapped)?;
    SecretKey::from_slice(&bytes).ok()
}

fn secret_aad(key: &str) -> String {
    format!("{SECRET_AAD_PREFIX}{key}")
}

fn wrap_aad(kind: &str) -> String {
    format!("{WRAP_AAD_PREFIX}{kind}")
}

/// Argon2id settings stored next to the password wrap, so the cost can be
/// raised later without breaking existing databases.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KdfParams {
    pub algorithm: String,
    pub salt: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub(crate) fn generate() -> Self {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        Self {
            algorithm: "argon2id-v19".to_string(),
            salt: BASE64.encode(salt),
            memory_kib: 19_456,
            iterations: 3,
            parallelism: 1,
        }
    }

    pub(crate) fn derive(&self, password: &str) -> Result<SecretKey, String> {
        if self.algorithm != "argon2id-v19" {
            return Err(format!("Unsupported key derivation '{}'", self.algorithm));
        }
        let salt = BASE64
            .decode(self.salt.as_bytes())
            .map_err(|_| "Stored key derivation salt is not valid base64".to_string())?;
        derive_argon2id(
            password.as_bytes(),
            &salt,
            self.memory_kib,
            self.iterations,
            self.parallelism,
        )
    }
}

pub(crate) fn derive_argon2id(
    password: &[u8],
    salt: &[u8],
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<SecretKey, String> {
    let params = Params::new(memory_kib, iterations, parallelism, Some(KEY_LEN))
        .map_err(|e| format!("Invalid Argon2 parameters: {e}"))?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password, salt, &mut key[..])
        .map_err(|e| format!("Failed to derive key: {e}"))?;
    Ok(SecretKey(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_are_bound_to_their_key() {
        let data_key = SecretKey::generate();
        let sealed = encrypt_secret(&data_key, "profile-1", "hunter2").unwrap();
        let plaintext =
            decrypt_secret(&data_key, "profile-1", &sealed.nonce, &sealed.ciphertext).unwrap();
        assert_eq!(plaintext.as_str(), "hunter2");
        assert!(decrypt_secret(&data_key, "profile-2", &sealed.nonce, &sealed.ciphertext).is_err());
        assert!(decrypt_secret(
            &SecretKey::generate(),
            "profile-1",
            &sealed.nonce,
            &sealed.ciphertext
        )
        .is_err());
    }

    #[test]
    fn data_key_unwraps_only_with_the_right_key_and_kind() {
        let data_key = SecretKey::generate();
        let wrapping_key = SecretKey::generate();
        let wrapped = wrap_data_key(&wrapping_key, "system", &data_key).unwrap();
        let unwrapped =
            unwrap_data_key(&wrapping_key, "system", &wrapped.nonce, &wrapped.ciphertext).unwrap();
        assert_eq!(unwrapped.0[..], data_key.0[..]);
        assert!(unwrap_data_key(
            &wrapping_key,
            "password",
            &wrapped.nonce,
            &wrapped.ciphertext
        )
        .is_none());
        assert!(unwrap_data_key(
            &SecretKey::generate(),
            "system",
            &wrapped.nonce,
            &wrapped.ciphertext
        )
        .is_none());
    }

    #[test]
    fn password_derivation_depends_on_password_and_salt() {
        let params = KdfParams {
            memory_kib: 64,
            iterations: 1,
            ..KdfParams::generate()
        };
        let first = params.derive("correct horse").unwrap();
        assert_eq!(first.0[..], params.derive("correct horse").unwrap().0[..]);
        assert_ne!(first.0[..], params.derive("wrong horse").unwrap().0[..]);
        let other_salt = KdfParams {
            salt: KdfParams::generate().salt,
            ..params.clone()
        };
        assert_ne!(
            first.0[..],
            other_salt.derive("correct horse").unwrap().0[..]
        );
    }

    #[test]
    fn base64_round_trip() {
        let key = SecretKey::generate();
        let restored = SecretKey::from_base64(&key.to_base64()).unwrap();
        assert_eq!(key.0[..], restored.0[..]);
        assert!(SecretKey::from_base64("c2hvcnQ=").is_err());
    }
}
