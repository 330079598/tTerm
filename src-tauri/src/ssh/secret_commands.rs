use super::secret_store::{SecretBackendStatus, SecretStoreState, StrongholdPasswordInput};
use tauri::{AppHandle, State};

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
