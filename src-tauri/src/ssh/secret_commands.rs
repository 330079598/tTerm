use super::secret_store::{
    types::ChangeVaultPasswordInput, types::SecretBackendStatus, types::SecretStoreState,
    types::VaultPasswordInput, verification::VerificationPurpose,
};
use serde::{Deserialize, Serialize};
use tterm_core::core::blocking::run_blocking;
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
pub struct VerifyUserInput {
    pub purpose: VerificationPurpose,
    /// The master password, when that is how this device verifies.
    #[serde(default)]
    pub password: Option<String>,
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
pub async fn get_secret_backend_status(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    get_secret_backend_status_service(secret_state).await
}
async fn get_secret_backend_status_service(
    secret_state: SecretStoreState,
) -> Result<SecretBackendStatus, String> {
    // Waits for startup to unlock saved passwords, which may sit on a
    // credential store prompt; keep that off the main thread.
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.get_status()).await
}

#[tauri::command]
pub async fn unlock_secret_vault(
    app: tauri::AppHandle,
    input: VaultPasswordInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    let data_dir = crate::app_paths::get(&app)?.data_dir.clone();
    unlock_secret_vault_service(secret_state, data_dir, input).await
}
async fn unlock_secret_vault_service(
    secret_state: SecretStoreState,
    data_dir: std::path::PathBuf,
    input: VaultPasswordInput,
) -> Result<SecretBackendStatus, String> {
    // Key derivation takes a noticeable moment; keep it off the main thread.
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.unlock(&data_dir, input)).await
}

#[tauri::command]
pub async fn lock_secret_vault(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    lock_secret_vault_service(secret_state).await
}
async fn lock_secret_vault_service(
    secret_state: SecretStoreState,
) -> Result<SecretBackendStatus, String> {
    // Waits for startup to unlock saved passwords; keep that off the main thread.
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.lock()).await
}

#[tauri::command]
pub async fn retry_system_unlock(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    retry_system_unlock_service(secret_state).await
}
async fn retry_system_unlock_service(
    secret_state: SecretStoreState,
) -> Result<SecretBackendStatus, String> {
    // The credential store may sit on a prompt; keep that off the main thread.
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.retry_system_unlock()).await
}

#[tauri::command]
pub async fn reset_saved_passwords(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    reset_saved_passwords_service(secret_state).await
}
async fn reset_saved_passwords_service(
    secret_state: SecretStoreState,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.reset_saved_passwords()).await
}

#[tauri::command]
pub async fn change_vault_password(
    input: ChangeVaultPasswordInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    change_vault_password_service(secret_state, input).await
}
async fn change_vault_password_service(
    secret_state: SecretStoreState,
    input: ChangeVaultPasswordInput,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.change_master_password(input)).await
}

#[tauri::command]
pub async fn set_master_password(
    input: MasterPasswordInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    set_master_password_service(secret_state, input).await
}
async fn set_master_password_service(
    secret_state: SecretStoreState,
    input: MasterPasswordInput,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        let password = Zeroizing::new(input.password);
        secret_state.set_master_password(&password)
    })
    .await
}

#[tauri::command]
pub async fn remove_master_password(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    remove_master_password_service(secret_state).await
}
async fn remove_master_password_service(
    secret_state: SecretStoreState,
) -> Result<SecretBackendStatus, String> {
    // Waits for startup to unlock saved passwords; keep that off the main thread.
    let secret_state = secret_state.clone();
    run_blocking(move || secret_state.remove_master_password()).await
}

#[tauri::command]
pub async fn set_secret_storage_mode(
    input: SecretStorageModeInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SecretBackendStatus, String> {
    let secret_state = services.shared.secrets.clone();
    set_secret_storage_mode_service(secret_state, input).await
}
async fn set_secret_storage_mode_service(
    secret_state: SecretStoreState,
    input: SecretStorageModeInput,
) -> Result<SecretBackendStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        let password = input.password.map(Zeroizing::new);
        secret_state.set_storage_mode(&input.mode, password.as_deref().map(String::as_str))
    })
    .await
}

/// Confirms it is the user before saved passwords are shown or sent
/// somewhere new; the operation that asked for it is then retried.
#[tauri::command]
pub async fn verify_user(
    input: VerifyUserInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let secret_state = services.shared.secrets.clone();
    let verifier = services.shared.platform.verification.clone();
    verify_user_service(secret_state, verifier, input).await
}
async fn verify_user_service(
    secret_state: SecretStoreState,
    verifier: std::sync::Arc<dyn tterm_platform::verification::UserVerification>,
    input: VerifyUserInput,
) -> Result<(), String> {
    // Waits on the OS prompt or on key derivation.
    let secret_state = secret_state.clone();
    let verifier = verifier;
    run_blocking(move || {
        let password = input.password.map(Zeroizing::new);
        secret_state.verify_user(
            verifier.as_ref(),
            input.purpose,
            password.as_deref().map(String::as_str),
        )
    })
    .await
}

#[tauri::command]
pub async fn list_saved_secrets(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Vec<SavedSecretEntry>, String> {
    let secret_state = services.shared.secrets.clone();
    list_saved_secrets_service(secret_state).await
}
async fn list_saved_secrets_service(
    secret_state: SecretStoreState,
) -> Result<Vec<SavedSecretEntry>, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        let saved = secret_state
            .saved_keys()?
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut entries = Vec::new();
        for summary in crate::profiles::storage::saved_secret_summaries()? {
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
    })
    .await
}

#[tauri::command]
pub async fn get_saved_secret(
    input: GetSavedSecretInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<String, String> {
    let secret_state = services.shared.secrets.clone();
    get_saved_secret_service(secret_state, input).await
}
async fn get_saved_secret_service(
    secret_state: SecretStoreState,
    input: GetSavedSecretInput,
) -> Result<String, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        let key = input.key.trim();
        if key.is_empty() {
            return Err("Secret key is required".to_string());
        }

        let allowed = crate::profiles::storage::saved_secret_keys()?
            .into_iter()
            .any(|candidate| candidate == key);
        if !allowed {
            return Err("Saved secret is not linked to a current profile.".to_string());
        }

        if !secret_state.unlocked()? {
            return Err("Saved passwords are locked. Unlock them first.".to_string());
        }
        secret_state.require_verification(VerificationPurpose::Reveal)?;
        secret_state
            .get_password(key)?
            .ok_or_else(|| "Saved password is no longer available.".to_string())
    })
    .await
}

#[tauri::command]
pub async fn delete_saved_secret(
    input: DeleteSavedSecretInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    delete_saved_secret_service(secret_state, input).await
}
async fn delete_saved_secret_service(
    secret_state: SecretStoreState,
    input: DeleteSavedSecretInput,
) -> Result<bool, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        let key = input.key.trim();
        if key.is_empty() {
            return Err("Secret key is required".to_string());
        }

        let allowed = crate::profiles::storage::saved_secret_keys()?
            .into_iter()
            .any(|candidate| candidate == key);
        if !allowed {
            return Err("Saved secret is not linked to a current profile.".to_string());
        }

        secret_state.delete_password(key)
    })
    .await
}
