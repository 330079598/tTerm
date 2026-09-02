use super::secret_store::{
    ChangeVaultPasswordInput, SecretBackendStatus, SecretStoreState, VaultPasswordInput,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStorageModeInput {
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySecretStoreInput {
    pub direction: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopySecretStoreResult {
    pub copied: usize,
    pub skipped: usize,
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
pub fn unlock_secret_vault(
    app: AppHandle,
    input: VaultPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.unlock_vault(&app, input)
}

#[tauri::command]
pub fn lock_secret_vault(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.lock_vault()
}

#[tauri::command]
pub fn change_vault_password(
    app: AppHandle,
    input: ChangeVaultPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.change_vault_password(&app, input)
}

#[tauri::command]
pub fn set_secret_vault_enabled(
    enabled: bool,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.set_vault_enabled(enabled)
}

#[tauri::command]
pub fn set_secret_storage_mode(
    input: SecretStorageModeInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.set_storage_mode(&input.mode)
}

#[tauri::command]
pub fn copy_secret_store(
    app: AppHandle,
    input: CopySecretStoreInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<CopySecretStoreResult, String> {
    let (from, to) = match input.direction.as_str() {
        "systemToVault" => ("system", "vault"),
        "vaultToSystem" => ("vault", "system"),
        _ => return Err("Unsupported secret copy direction".to_string()),
    };

    if (from == "system" || to == "system") && !secret_state.keyring_available()? {
        return Err("System credential store is unavailable.".to_string());
    }
    if (from == "vault" || to == "vault") && !secret_state.vault_unlocked()? {
        return Err("Unlock the app vault before copying passwords.".to_string());
    }

    let mut keys = crate::profiles::saved_secret_keys()?;
    keys.sort();
    keys.dedup();

    let mut copied = 0;
    let mut skipped = 0;
    for key in keys {
        if secret_state.copy_password(&app, &key, from, to)? {
            copied += 1;
        } else {
            skipped += 1;
        }
    }

    Ok(CopySecretStoreResult { copied, skipped })
}

#[tauri::command]
pub fn list_saved_secrets(
    app: AppHandle,
    secret_state: State<'_, SecretStoreState>,
) -> Result<Vec<SavedSecretEntry>, String> {
    let mut entries = Vec::new();
    for summary in crate::profiles::saved_secret_summaries()? {
        if secret_state.has_password(&app, &summary.key)? {
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
