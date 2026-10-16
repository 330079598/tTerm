//! The key vault: SSH private keys kept in the database, encrypted like the
//! saved passwords, instead of in key files.
//!
//! Each key is one saved secret under `ssh-key:<id>`, so password sync and
//! encrypted backups take it to other devices along with the passwords. A
//! profile signs in with a vault key by having `tterm-key:<id>` as its
//! private key path, which [`crate::ssh::key_file::read_key`] resolves
//! wherever a key is loaded.

mod entry;
mod export;
mod import;
pub(crate) mod store;
#[cfg(test)]
mod tests;

use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::types::SecretStoreState;
use crate::ssh::secret_store::verification::VerificationPurpose;
use entry::VaultKey;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;
use tauri::State;

/// The start of a private key path that names a key in the vault.
pub(crate) const REFERENCE_PREFIX: &str = "tterm-key:";
/// The start of the saved secret a vault key is sealed in.
const SECRET_PREFIX: &str = "ssh-key:";
/// Key and certificate files are a few KiB at most; anything bigger is not one.
const MAX_FILE_BYTES: u64 = 64 * 1024;

/// The app's saved password state, for loading keys deep inside connection
/// code that has no app handle to ask for it.
static SECRET_STORE: OnceLock<SecretStoreState> = OnceLock::new();

/// A key in the vault as listed in the settings and the connection dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultKeySummary {
    pub id: String,
    pub name: String,
    /// The key type, such as `ssh-ed25519`.
    pub algorithm: String,
    /// `SHA256:…`, as `ssh-keygen -l` shows it.
    pub fingerprint: String,
    /// The public key line for `authorized_keys`.
    pub public_key: String,
    /// The key itself is protected by a passphrase.
    pub encrypted: bool,
    /// Its passphrase is kept with it, so connecting asks for none.
    pub passphrase_saved: bool,
    pub has_certificate: bool,
    /// When the certificate expires (unix seconds); `None` for one that never does.
    pub certificate_valid_before: Option<u64>,
    /// Unix milliseconds.
    pub created_at: i64,
    /// The profiles that sign in with the key.
    pub used_by: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportVaultKeyInput {
    /// The private key file.
    pub path: String,
    /// The file name when empty.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
    /// Keep the passphrase with the key.
    #[serde(default)]
    pub remember_passphrase: bool,
    #[serde(default)]
    pub certificate_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameVaultKeyInput {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVaultKeyCertificateInput {
    pub id: String,
    /// The new certificate file; `None` removes the certificate.
    #[serde(default)]
    pub certificate_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultKeyIdInput {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportVaultKeyInput {
    pub id: String,
    /// Where to write the private key.
    pub path: String,
}

/// Makes the vault readable to key loading; called once at startup.
pub(crate) fn install(state: SecretStoreState) {
    // A second call would only hand over the same state again.
    let _ = SECRET_STORE.set(state);
}

/// The vault key id a private key path names, if it names one.
pub(crate) fn reference_id(path: &str) -> Option<&str> {
    path.trim()
        .strip_prefix(REFERENCE_PREFIX)
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn secret_key(id: &str) -> String {
    format!("{SECRET_PREFIX}{id}")
}

/// Reads key `id` to sign in with. `subject` starts the messages, e.g. "SSH key".
pub(crate) fn load(id: &str, subject: &str) -> Result<VaultKey, String> {
    let state = SECRET_STORE.get().ok_or_else(|| {
        format!("{subject} is in the key vault, which is not ready yet. Try again in a moment.")
    })?;
    let data_key = state.data_key().map_err(|reason| {
        format!("{subject} is in the key vault, which cannot be read: {reason}")
    })?;
    crate::db::read(|connection| store::get(connection, &data_key, id))?.ok_or_else(|| {
        format!(
            "{subject} is not in this device's key vault. Sync or restore the saved passwords \
             of the device that has it, or choose another key."
        )
    })
}

fn summarize_all(state: &SecretStoreState) -> Result<Vec<VaultKeySummary>, String> {
    let data_key = state.data_key()?;
    let profiles = crate::profiles::storage::load_profiles()?;
    let mut summaries = crate::db::read(|connection| {
        let mut summaries = Vec::new();
        for id in store::ids(connection)? {
            if let Some(key) = store::get(connection, &data_key, &id)? {
                summaries.push(key.summary(&id, store::used_by(&profiles, &id)));
            }
        }
        Ok(summaries)
    })?;
    summaries.sort_by_key(|summary| summary.name.to_lowercase());
    Ok(summaries)
}

/// Changes key `id` in one transaction and returns its new summary.
fn update(
    state: &SecretStoreState,
    id: &str,
    change: impl FnOnce(&mut VaultKey) -> Result<(), String>,
) -> Result<VaultKeySummary, String> {
    let data_key = state.data_key()?;
    let profiles = crate::profiles::storage::load_profiles()?;
    crate::db::write(|transaction| {
        let mut key = store::get(transaction, &data_key, id)?
            .ok_or_else(|| "The key is no longer in the key vault.".to_string())?;
        change(&mut key)?;
        store::put(transaction, &data_key, id, &key)?;
        Ok(key.summary(id, store::used_by(&profiles, id)))
    })
}

#[tauri::command]
pub async fn list_vault_keys(
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<VaultKeySummary>, String> {
    let state = secret_state.inner().clone();
    run_blocking(move || summarize_all(&state)).await
}

/// Imports a key file into the vault.
#[tauri::command]
pub async fn import_vault_key(
    input: ImportVaultKeyInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<VaultKeySummary, String> {
    let state = secret_state.inner().clone();
    run_blocking(move || {
        let data_key = state.data_key()?;
        let key = import::read_import(&input)?;
        let id = uuid::Uuid::new_v4().to_string();
        crate::db::write(|transaction| {
            import::ensure_new(transaction, &data_key, &key)?;
            store::put(transaction, &data_key, &id, &key)
        })?;
        Ok(key.summary(&id, Vec::new()))
    })
    .await
}

#[tauri::command]
pub async fn rename_vault_key(
    input: RenameVaultKeyInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<VaultKeySummary, String> {
    let state = secret_state.inner().clone();
    run_blocking(move || {
        let name = import::checked_name(&input.name)?;
        update(&state, &input.id, |key| {
            key.name = name;
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn set_vault_key_certificate(
    input: SetVaultKeyCertificateInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<VaultKeySummary, String> {
    let state = secret_state.inner().clone();
    run_blocking(move || {
        let path = input
            .certificate_path
            .filter(|path| !path.trim().is_empty());
        update(&state, &input.id, |key| {
            key.certificate = match path {
                Some(path) => {
                    let public_key = russh::keys::PublicKey::from_openssh(&key.public_key)
                        .map_err(|error| format!("The key's public half is damaged: {error}"))?;
                    Some(import::read_certificate(
                        Path::new(path.trim()),
                        &public_key,
                    )?)
                }
                None => None,
            };
            Ok(())
        })
    })
    .await
}

#[tauri::command]
pub async fn delete_vault_key(
    input: VaultKeyIdInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<bool, String> {
    let state = secret_state.inner().clone();
    run_blocking(move || {
        // Deleting needs the vault unlocked, as every other change does.
        state.data_key()?;
        crate::db::write(|transaction| store::delete(transaction, &input.id))
    })
    .await
}

/// Writes the private key, and its certificate, out as files. The key
/// leaves the vault in plain form, so the user must confirm it is them.
#[tauri::command]
pub async fn export_vault_key(
    input: ExportVaultKeyInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<(), String> {
    let state = secret_state.inner().clone();
    run_blocking(move || {
        let data_key = state.data_key()?;
        state.require_verification(VerificationPurpose::Sensitive)?;
        let key = crate::db::read(|connection| store::get(connection, &data_key, &input.id))?
            .ok_or_else(|| "The key is no longer in the key vault.".to_string())?;
        export::write_key_files(Path::new(input.path.trim()), &key)
    })
    .await
}
