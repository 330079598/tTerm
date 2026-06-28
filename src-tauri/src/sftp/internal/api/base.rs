use crate::core::session::{PtyConnectionOptions, SessionPlan};
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::{SftpConnectionPool, SftpDirectoryEntry, SftpDirectoryListing};
use crate::sftp::store;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use tauri::{AppHandle, State};

fn push_unique_candidate(candidates: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if !path.trim().is_empty() && !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn initial_directory_candidates(plan: &SessionPlan) -> Result<Vec<String>, String> {
    let host = plan.host.as_ref().ok_or("Host is required")?;
    let username = plan.username.as_ref().ok_or("Username is required")?;

    let mut candidates = Vec::new();
    if let Ok(Some(last_path)) = store::get_last_directory(host, plan.port, username) {
        push_unique_candidate(&mut candidates, last_path);
    }
    push_unique_candidate(&mut candidates, ".");
    push_unique_candidate(&mut candidates, "/");

    Ok(candidates)
}

async fn resolve_initial_directory(
    sftp: &SftpSession,
    plan: &SessionPlan,
) -> Result<String, String> {
    let mut last_error = None;

    for path in initial_directory_candidates(plan)? {
        match sftp.canonicalize(&path).await {
            Ok(canonical) => match sftp.read_dir(&canonical).await {
                Ok(_) => return Ok(canonical),
                Err(err) => last_error = Some(map_sftp_error(err)),
            },
            Err(err) => last_error = Some(map_sftp_error(err)),
        }
    }

    match last_error {
        Some(error) => Err(format!(
            "Failed to access any directory (last saved, home, or root): {error}"
        )),
        None => Err("Failed to access any directory (last saved, home, or root)".to_string()),
    }
}

async fn read_directory_entries(
    sftp: &SftpSession,
    current_path: &str,
) -> Result<Vec<SftpDirectoryEntry>, String> {
    let mut entries = sftp
        .read_dir(current_path)
        .await
        .map_err(map_sftp_error)?
        .map(|entry| {
            let name = entry.file_name();
            let metadata = entry.metadata();

            SftpDirectoryEntry {
                path: crate::sftp::internal::paths::join_remote_path(current_path, &name),
                name,
                is_dir: metadata.is_dir(),
                is_symlink: metadata.is_symlink(),
                size: metadata.size,
                modified_at: metadata.mtime.map(|value| value as i64 * 1000),
                permissions: metadata
                    .permissions
                    .map(|_| metadata.permissions().to_string()),
                owner: metadata
                    .user
                    .clone()
                    .or_else(|| metadata.uid.map(|value| value.to_string())),
                group: metadata
                    .group
                    .clone()
                    .or_else(|| metadata.gid.map(|value| value.to_string())),
            }
        })
        .collect::<Vec<_>>();

    crate::sftp::internal::paths::sort_entries(&mut entries);
    Ok(entries)
}

#[tauri::command]
pub async fn sftp_list_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: Option<String>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<SftpDirectoryListing, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let requested_path = crate::sftp::internal::paths::sanitize_target_path(path);

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        let current_path = match requested_path {
            Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
            None => resolve_initial_directory(sftp, &plan).await?,
        };

        let entries = read_directory_entries(sftp, &current_path).await?;

        // Save the current path as the last directory
        let host = plan.host.as_ref().ok_or("Host is required")?;
        let username = plan.username.as_ref().ok_or("Username is required")?;
        let normalized_path = crate::sftp::internal::paths::normalize_remote_path(&current_path);
        let _ = store::save_last_directory(host, plan.port, username, &normalized_path);

        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: crate::sftp::internal::paths::parent_remote_path(&current_path),
            entries,
        })
    })
}

#[tauri::command]
pub async fn sftp_create_directory(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        sftp.create_dir(path).await.map_err(map_sftp_error)
    })
}

#[tauri::command]
pub async fn sftp_rename_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    old_path: String,
    new_path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        sftp.rename(old_path, new_path).await.map_err(map_sftp_error)
    })
}
