//! One key in the key vault as it is stored, and what is read from a key's
//! text when it comes in.

use super::VaultKeySummary;
use crate::ssh::rsa_signer::RingRsaSigner;
use russh::keys::{self, Certificate, HashAlg, PrivateKey, PublicKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// A key as it is sealed in its saved secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultKey {
    pub name: String,
    /// The key file's text exactly as imported, so an export gives the file back.
    pub private_key: String,
    /// The public half in OpenSSH form, kept so listing the vault needs no
    /// passphrase even for a key that has one.
    pub public_key: String,
    /// The passphrase the user chose to keep with the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    /// The key's OpenSSH certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    /// Unix milliseconds.
    pub created_at: i64,
}

impl Drop for VaultKey {
    fn drop(&mut self) {
        self.private_key.zeroize();
        if let Some(passphrase) = self.passphrase.as_mut() {
            passphrase.zeroize();
        }
    }
}

impl VaultKey {
    pub(super) fn summary(&self, id: &str, used_by: Vec<String>) -> VaultKeySummary {
        let public_key = PublicKey::from_openssh(&self.public_key).ok();
        let certificate = self
            .certificate
            .as_deref()
            .and_then(|text| Certificate::from_openssh(text.trim()).ok());
        VaultKeySummary {
            id: id.to_string(),
            name: self.name.clone(),
            algorithm: public_key
                .as_ref()
                .map(|key| key.algorithm().to_string())
                .unwrap_or_default(),
            fingerprint: public_key
                .as_ref()
                .map(|key| key.fingerprint(HashAlg::Sha256).to_string())
                .unwrap_or_default(),
            public_key: self.public_key.clone(),
            encrypted: key_is_encrypted(&self.private_key),
            passphrase_saved: self.passphrase.is_some(),
            has_certificate: self.certificate.is_some(),
            // OpenSSH writes "forever" as the largest time there is.
            certificate_valid_before: certificate
                .map(|certificate| certificate.valid_before())
                .filter(|seconds| *seconds != u64::MAX),
            created_at: self.created_at,
            used_by,
        }
    }
}

/// What importing a key learns from its text.
pub(super) struct KeyInfo {
    pub public_key: PublicKey,
    pub encrypted: bool,
}

/// Reads the public half of a private key, in any format a connection can
/// sign in with. `passphrase` is checked against an encrypted key; a key in
/// another format than OpenSSH's needs it to show its public half at all.
pub(super) fn inspect(text: &str, passphrase: Option<&str>) -> Result<KeyInfo, String> {
    if let Ok(key) = PrivateKey::from_openssh(text) {
        if let (true, Some(passphrase)) = (key.is_encrypted(), passphrase) {
            key.decrypt(passphrase.as_bytes())
                .map_err(|_| "That passphrase does not unlock the key.".to_string())?;
        }
        return Ok(KeyInfo {
            public_key: key.public_key().clone(),
            encrypted: key.is_encrypted(),
        });
    }
    match keys::decode_secret_key(text, None) {
        Ok(key) => Ok(KeyInfo {
            public_key: key.public_key().clone(),
            encrypted: false,
        }),
        Err(keys::Error::KeyIsEncrypted) => {
            let passphrase = passphrase
                .ok_or_else(|| "The key is encrypted; enter its passphrase.".to_string())?;
            let key = keys::decode_secret_key(text, Some(passphrase))
                .map_err(|_| "That passphrase does not unlock the key.".to_string())?;
            Ok(KeyInfo {
                public_key: key.public_key().clone(),
                encrypted: true,
            })
        }
        Err(error) => match RingRsaSigner::from_pem(text) {
            Some(Ok((_, public_key))) => Ok(KeyInfo {
                public_key,
                encrypted: false,
            }),
            Some(Err(reason)) => Err(format!("The key cannot be used: {reason}")),
            None => Err(format!("The file is not a private key: {error}")),
        },
    }
}

/// Whether the key needs a passphrase, without one at hand.
fn key_is_encrypted(text: &str) -> bool {
    match PrivateKey::from_openssh(text) {
        Ok(key) => key.is_encrypted(),
        Err(_) => matches!(
            keys::decode_secret_key(text, None),
            Err(keys::Error::KeyIsEncrypted)
        ),
    }
}
