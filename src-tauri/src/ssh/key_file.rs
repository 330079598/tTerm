//! Reading private keys, from key files or from the key vault, and the
//! certificates that go with them.

use crate::ssh::key_vault;
use russh::keys::{self, Certificate, PrivateKey, PublicKey};
use std::io::ErrorKind;
use zeroize::Zeroizing;

/// A private key's text and what the key vault keeps with it.
pub struct KeyMaterial {
    /// What messages call the key: its file path, or its name in the vault.
    pub label: String,
    pub text: Zeroizing<String>,
    /// The passphrase kept with a vault key.
    pub passphrase: Option<Zeroizing<String>>,
    /// The certificate kept with a vault key.
    pub certificate: Option<String>,
}

impl KeyMaterial {
    /// The passphrase to try when the profile gives none.
    pub fn saved_passphrase(&self) -> Option<&str> {
        self.passphrase.as_deref().map(String::as_str)
    }

    /// Decodes the key with `passphrase`, or with the saved one when that is
    /// `None`.
    pub fn decode(&self, passphrase: Option<&str>, subject: &str) -> Result<PrivateKey, String> {
        keys::decode_secret_key(&self.text, passphrase.or(self.saved_passphrase()))
            .map_err(|error| describe_key_error(subject, &self.label, &error))
    }
}

/// Reads the private key a profile's key path names: a key file, or a key in
/// the key vault when the path is a `tterm-key:` reference. `subject` says
/// whose key it is, e.g. "SSH key" or "Jump host SSH key".
pub fn read_key(path: &str, subject: &str) -> Result<KeyMaterial, String> {
    if let Some(id) = key_vault::reference_id(path) {
        let mut key = key_vault::load(id, subject)?;
        return Ok(KeyMaterial {
            label: format!("\"{}\" from the key vault", key.name),
            text: Zeroizing::new(std::mem::take(&mut key.private_key)),
            passphrase: key.passphrase.take().map(Zeroizing::new),
            certificate: key.certificate.take(),
        });
    }
    std::fs::read_to_string(path)
        .map(|text| KeyMaterial {
            label: path.to_string(),
            text: Zeroizing::new(text),
            passphrase: None,
            certificate: None,
        })
        .map_err(|error| describe_key_error(subject, path, &keys::Error::IO(error)))
}

/// Whether the key a profile's key path names is on this device.
pub fn key_exists(path: &str) -> bool {
    match key_vault::reference_id(path) {
        Some(id) => {
            crate::db::read(|connection| key_vault::store::exists(connection, id)).unwrap_or(false)
        }
        None => std::path::Path::new(path).is_file(),
    }
}

/// How messages name the key at `path` when its name is not at hand.
pub fn display_path(path: &str) -> &str {
    if key_vault::reference_id(path).is_some() {
        "from the key vault"
    } else {
        path
    }
}

pub fn describe_key_error(subject: &str, path: &str, error: &keys::Error) -> String {
    match error {
        keys::Error::IO(io) if io.kind() == ErrorKind::NotFound => {
            format!("{subject} file not found: {path}")
        }
        keys::Error::IO(io) if io.kind() == ErrorKind::PermissionDenied => {
            format!("Permission denied reading {subject} file: {path}")
        }
        keys::Error::IO(io) => format!("Cannot read {subject} file {path}: {io}"),
        keys::Error::KeyIsEncrypted => {
            format!("{subject} {path} is encrypted; enter its passphrase")
        }
        other => format!("Failed to load {subject} {path}: {other}"),
    }
}

/// Reads an OpenSSH certificate, which must certify `public_key` and must not
/// have expired. `label` names it in messages that start with "certificate".
pub fn parse_certificate(
    text: &str,
    label: &str,
    public_key: &PublicKey,
) -> Result<Certificate, String> {
    let certificate = Certificate::from_openssh(text.trim())
        .map_err(|error| format!("certificate {label} is not an OpenSSH certificate: {error}"))?;
    if certificate.public_key() != public_key.key_data() {
        return Err(format!(
            "certificate {label} certifies a different key than the private key"
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    if certificate.valid_before() <= now {
        let expired = i64::try_from(certificate.valid_before())
            .ok()
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| {
                chrono::DateTime::<chrono::Local>::from(time)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        return Err(format!(
            "certificate {label} expired at {expired}; get a new one from your certificate authority"
        ));
    }
    Ok(certificate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_file_names_the_path() {
        let path = "/nonexistent/tterm/id_ed25519";
        let error = read_key(path, "SSH key").err().expect("missing file");
        assert_eq!(error, format!("SSH key file not found: {path}"));
    }

    #[test]
    fn unparseable_key_file_names_the_path() {
        let path = std::env::temp_dir().join(format!("tterm-bad-key-{}", std::process::id()));
        std::fs::write(&path, "not a key").expect("write fixture");
        let path_str = path.to_string_lossy().into_owned();
        let error = read_key(&path_str, "Jump host SSH key")
            .and_then(|key| key.decode(None, "Jump host SSH key"))
            .expect_err("bad key");
        assert!(
            error.starts_with(&format!("Failed to load Jump host SSH key {path_str}:")),
            "{error}"
        );
        std::fs::remove_file(path).ok();
    }
}
