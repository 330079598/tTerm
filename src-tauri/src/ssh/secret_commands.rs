use super::secret_store::{
    ChangeVaultPasswordInput, SecretBackendStatus, SecretStoreState, VaultPasswordInput,
};
use crate::core::blocking::run_blocking;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStorageModeInput {
    pub mode: String,
    /// The master password to set when switching to `password` mode.
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterPasswordInput {
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSavedSecretInput {
    pub key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSavedSecretInput {
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSecretEntry {
    pub key: String,
    pub profile_id: String,
    pub profile_name: String,
    pub label: String,
    pub kind: String,
}

#[tauri::command]
pub fn get_secret_backend_status(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.get_status()
}

#[tauri::command]
pub async fn unlock_secret_vault(
    app: AppHandle,
    input: VaultPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    // Key derivation takes a noticeable moment; keep it off the main thread.
    let secret_state = secret_state.inner().clone();
    run_blocking(move || secret_state.unlock(&app, input)).await
}

#[tauri::command]
pub fn lock_secret_vault(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.lock()
}

#[tauri::command]
pub async fn change_vault_password(
    input: ChangeVaultPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || secret_state.change_master_password(input)).await
}

#[tauri::command]
pub async fn set_master_password(
    input: MasterPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        let password = Zeroizing::new(input.password);
        secret_state.set_master_password(&password)
    })
    .await
}

#[tauri::command]
pub fn remove_master_password(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.remove_master_password()
}

#[tauri::command]
pub async fn set_secret_storage_mode(
    input: SecretStorageModeInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        let password = input.password.map(Zeroizing::new);
        secret_state.set_storage_mode(&input.mode, password.as_deref().map(String::as_str))
    })
    .await
}

#[tauri::command]
pub fn list_saved_secrets(
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<SavedSecretEntry>, String> {
    let saved = secret_state
        .saved_keys()?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut entries = Vec::new();
    for summary in crate::profiles::saved_secret_summaries()? {
        if saved.contains(&summary.key) {
            entries.push(SavedSecretEntry {
                key: summary.key,
                profile_id: summary.profile_id,
                profile_name: summary.profile_name,
                label: summary.label,
                kind: summary.kind,
            });
        }
    }

    entries.sort_by(|left, right| {
        left.profile_name
            .to_lowercase()
            .cmp(&right.profile_name.to_lowercase())
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.key.cmp(&right.key))
    });
    entries.dedup_by(|left, right| left.key == right.key);
    Ok(entries)
}

#[tauri::command]
pub fn get_saved_secret(
    app: AppHandle,
    input: GetSavedSecretInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<String, String> {
    let key = input.key.trim();
    if key.is_empty() {
        return Err("Secret key is required".to_string());
    }

    let allowed = crate::profiles::saved_secret_keys()?
        .into_iter()
        .any(|candidate| candidate == key);
    if !allowed {
        return Err("Saved secret is not linked to a current profile.".to_string());
    }

    if !secret_state.unlocked()? {
        return Err("Saved passwords are locked. Unlock them first.".to_string());
    }
    secret_state
        .get_password(&app, key)?
        .ok_or_else(|| "Saved password is no longer available.".to_string())
}

#[tauri::command]
pub fn delete_saved_secret(
    app: AppHandle,
    input: DeleteSavedSecretInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<bool, String> {
    let key = input.key.trim();
    if key.is_empty() {
        return Err("Secret key is required".to_string());
    }

    let allowed = crate::profiles::saved_secret_keys()?
        .into_iter()
        .any(|candidate| candidate == key);
    if !allowed {
        return Err("Saved secret is not linked to a current profile.".to_string());
    }

    secret_state.delete_password(&app, key)
}
