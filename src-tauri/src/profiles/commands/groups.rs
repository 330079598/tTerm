use tterm_core::profiles::group_service;
use tterm_core::profiles::types::ProfileGroup;

#[tauri::command]
pub async fn list_profile_groups() -> Result<Vec<ProfileGroup>, String> {
    group_service::list_profile_groups().await
}

#[tauri::command]
pub async fn create_profile_group(
    name: String,
    parent_id: Option<String>,
) -> Result<Vec<ProfileGroup>, String> {
    group_service::create_profile_group(name, parent_id).await
}

#[tauri::command]
pub async fn rename_profile_group(id: String, name: String) -> Result<Vec<ProfileGroup>, String> {
    group_service::rename_profile_group(id, name).await
}

#[tauri::command]
pub async fn move_profile_group(
    id: String,
    parent_id: Option<String>,
) -> Result<Vec<ProfileGroup>, String> {
    group_service::move_profile_group(id, parent_id).await
}

#[tauri::command]
pub async fn delete_profile_group(id: String) -> Result<Vec<ProfileGroup>, String> {
    group_service::delete_profile_group(id).await
}

#[tauri::command]
pub async fn move_profiles_to_group(
    ids: Vec<String>,
    group_id: Option<String>,
    target_id: Option<String>,
) -> Result<(), String> {
    group_service::move_profiles_to_group(ids, group_id, target_id).await
}
