use super::{
    ExportVaultKeyInput, ImportVaultKeyInput, RenameVaultKeyInput, SetVaultKeyCertificateInput,
    VaultKeyIdInput, VaultKeySummary, delete_vault_key_service, export_vault_key_service,
    import_vault_key_service, list_vault_keys_service, rename_vault_key_service,
    set_vault_key_certificate_service,
};

#[tauri::command]
pub async fn list_vault_keys(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Vec<VaultKeySummary>, String> {
    let secret_state = services.shared.secrets.clone();
    list_vault_keys_service(secret_state).await
}

/// Imports a key file into the vault.
#[tauri::command]
pub async fn import_vault_key(
    input: ImportVaultKeyInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<VaultKeySummary, String> {
    let secret_state = services.shared.secrets.clone();
    import_vault_key_service(secret_state, input).await
}

#[tauri::command]
pub async fn rename_vault_key(
    input: RenameVaultKeyInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<VaultKeySummary, String> {
    let secret_state = services.shared.secrets.clone();
    rename_vault_key_service(secret_state, input).await
}

#[tauri::command]
pub async fn set_vault_key_certificate(
    input: SetVaultKeyCertificateInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<VaultKeySummary, String> {
    let secret_state = services.shared.secrets.clone();
    set_vault_key_certificate_service(secret_state, input).await
}

#[tauri::command]
pub async fn delete_vault_key(
    input: VaultKeyIdInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    delete_vault_key_service(secret_state, input).await
}

/// Writes the private key, and its certificate, out as files. The key
/// leaves the vault in plain form, so the user must confirm it is them.
#[tauri::command]
pub async fn export_vault_key(
    input: ExportVaultKeyInput,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let secret_state = services.shared.secrets.clone();
    export_vault_key_service(secret_state, input).await
}
