use super::secret_store::{SecretBackendStatus, SecretStoreState, StrongholdPasswordInput};
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

#[tauri::command]
pub fn get_secret_backend_status(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.get_status()
}

#[tauri::command]
pub fn unlock_secret_vault(
    app: AppHandle,
    input: StrongholdPasswordInput,
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.unlock_stronghold(&app, input)
}

#[tauri::command]
pub fn lock_secret_vault(
    secret_state: State<'_, SecretStoreState>,
) -> Result<SecretBackendStatus, String> {
    secret_state.lock_stronghold()
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
