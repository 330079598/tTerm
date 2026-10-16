//! Bringing a key file, and the certificate that goes with it, into the vault.

use super::entry::{inspect, VaultKey};
use super::{store, ImportVaultKeyInput, MAX_FILE_BYTES};
use crate::ssh::secret_store::crypto::SecretKey as DataKey;
use rusqlite::Connection;
use russh::keys::{HashAlg, PublicKey};
use std::path::Path;
use zeroize::Zeroizing;

/// The longest name a key may have.
const MAX_NAME_CHARS: usize = 100;

/// Reads the key and certificate files of `input` into a new vault key.
pub(super) fn read_import(input: &ImportVaultKeyInput) -> Result<VaultKey, String> {
    let path = Path::new(input.path.trim());
    let text = read_small_file(path, "key")?;
    let passphrase = input
        .passphrase
        .as_deref()
        .filter(|value| !value.is_empty());
    let info = inspect(&text, passphrase)?;
    let name = match input.name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "SSH key".to_string()),
    };
    let certificate = input
        .certificate_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| read_certificate(Path::new(path), &info.public_key))
        .transpose()?;

    Ok(VaultKey {
        name: checked_name(&name)?,
        private_key: text.to_string(),
        public_key: info
            .public_key
            .to_openssh()
            .map_err(|error| format!("Failed to read the public key: {error}"))?,
        // A passphrase is only worth keeping when the key has one, and only
        // if the user asked for it.
        passphrase: passphrase
            .filter(|_| info.encrypted && input.remember_passphrase)
            .map(str::to_string),
        certificate,
        created_at: crate::ssh::store::now_unix_ms(),
    })
}

/// Fails when the vault already holds the same key, naming the copy there.
pub(super) fn ensure_new(
    connection: &Connection,
    data_key: &DataKey,
    key: &VaultKey,
) -> Result<(), String> {
    let fingerprint = |text: &str| {
        PublicKey::from_openssh(text)
            .ok()
            .map(|key| key.fingerprint(HashAlg::Sha256).to_string())
    };
    let incoming = fingerprint(&key.public_key);
    for id in store::ids(connection)? {
        let Some(existing) = store::get(connection, data_key, &id)? else {
            continue;
        };
        if incoming.is_some() && fingerprint(&existing.public_key) == incoming {
            return Err(format!(
                "This key is already in the key vault as \"{}\".",
                existing.name
            ));
        }
    }
    Ok(())
}

pub(super) fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the key.".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "The key name is too long; use at most {MAX_NAME_CHARS} characters."
        ));
    }
    Ok(name.to_string())
}

/// Reads the certificate at `path`, which must certify `public_key` and must
/// not have expired.
pub(super) fn read_certificate(path: &Path, public_key: &PublicKey) -> Result<String, String> {
    let text = read_small_file(path, "certificate")?;
    let label = path.to_string_lossy();
    crate::ssh::key_file::parse_certificate(&text, &label, public_key)?;
    Ok(text.trim().to_string())
}

/// Reads a key or certificate file, refusing anything too big to be one.
fn read_small_file(path: &Path, what: &str) -> Result<Zeroizing<String>, String> {
    let shown = path.to_string_lossy();
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("The {what} file was not found: {shown}")
        } else {
            format!("Cannot read the {what} file {shown}: {error}")
        }
    })?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("{shown} is too big to be an SSH {what}."));
    }
    std::fs::read_to_string(path)
        .map(Zeroizing::new)
        .map_err(|error| format!("Cannot read the {what} file {shown}: {error}"))
}
