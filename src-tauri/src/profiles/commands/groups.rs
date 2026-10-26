//! Commands that create, rename, move and delete profile groups, and file
//! profiles in them. Each returns what the groups page needs to redraw.

use crate::core::blocking::run_blocking;
use crate::profiles::groups::{create_group, delete_group, list_groups, move_group, rename_group};
use crate::profiles::storage::move_profiles;
use crate::profiles::types::ProfileGroup;

#[tauri::command]
pub async fn list_profile_groups() -> Result<Vec<ProfileGroup>, String> {
    run_blocking(|| crate::db::read(list_groups)).await
}

/// Creates a group under `parent_id`, or at the top.
#[tauri::command]
pub async fn create_profile_group(
    name: String,
    parent_id: Option<String>,
) -> Result<Vec<ProfileGroup>, String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            create_group(transaction, &name, parent_id.as_deref())?;
            list_groups(transaction)
        })
    })
    .await
}

#[tauri::command]
pub async fn rename_profile_group(id: String, name: String) -> Result<Vec<ProfileGroup>, String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            rename_group(transaction, &id, &name)?;
            list_groups(transaction)
        })
    })
    .await
}

/// Moves a group, with what it holds, under `parent_id` or to the top.
#[tauri::command]
pub async fn move_profile_group(
    id: String,
    parent_id: Option<String>,
) -> Result<Vec<ProfileGroup>, String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            move_group(transaction, &id, parent_id.as_deref())?;
            list_groups(transaction)
        })
    })
    .await
}

/// Deletes a group; its profiles and groups move up to its parent.
#[tauri::command]
pub async fn delete_profile_group(id: String) -> Result<Vec<ProfileGroup>, String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            delete_group(transaction, &id)?;
            list_groups(transaction)
        })
    })
    .await
}

/// Files profiles in `group_id` (ungrouped for none), before `target_id` or
/// last, in the order they had among themselves.
#[tauri::command]
pub async fn move_profiles_to_group(
    ids: Vec<String>,
    group_id: Option<String>,
    target_id: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        crate::db::write(|transaction| {
            if let Some(group_id) = group_id.as_deref() {
                if !list_groups(transaction)?
                    .iter()
                    .any(|group| group.id == group_id)
                {
                    return Err("This group no longer exists".to_string());
                }
            }
            move_profiles(transaction, &ids, group_id.as_deref(), target_id.as_deref())
        })
    })
    .await
}
