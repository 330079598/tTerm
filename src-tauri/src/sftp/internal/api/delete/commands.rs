use super::batch::{preview_delete_entries, run_delete_batch};
use super::*;
use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use std::future::Future;
use std::pin::Pin;
use tauri::{AppHandle, State};
#[tauri::command]
pub async fn sftp_preview_delete_entries(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    entries: Vec<DeleteEntryRequest>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<DeletePreviewResult, String> {
    if entries.is_empty() {
        return Err("No entries selected for deletion".to_string());
    }

    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    preview_delete_entries(
        &app,
        &tab_id,
        &plan,
        prompt_state.inner().clone(),
        pool_state.inner(),
        &entries,
    )
    .await
}

#[tauri::command]
pub async fn sftp_delete_entries(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    entries: Vec<DeleteEntryRequest>,
    options: DeleteOptions,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<DeleteBatchStartResult, String> {
    if entries.is_empty() {
        return Err("No entries selected for deletion".to_string());
    }

    for entry in &entries {
        if is_dangerous_delete_path(&entry.path) {
            return Err(format!("Refusing to delete unsafe path '{}'", entry.path));
        }
    }

    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let batch_id = next_delete_batch_id();
    tokio::spawn(run_delete_batch(
        app.clone(),
        tab_id,
        plan,
        prompt_state.inner().clone(),
        pool_state.inner().clone(),
        batch_id.clone(),
        entries,
        options,
    ));

    Ok(DeleteBatchStartResult { batch_id })
}

#[tauri::command]
pub async fn sftp_delete_entry(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    is_dir: bool,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        if is_dir {
            delete_directory_recursive(sftp, path).await
        } else {
            sftp.remove_file(path).await.map_err(map_sftp_error)
        }
    })
}

fn delete_directory_recursive<'a>(
    sftp: &'a SftpSession,
    path: String,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        // List all entries in the directory
        let entries = sftp.read_dir(&path).await.map_err(map_sftp_error)?;

        // Delete each entry recursively
        for entry in entries {
            let name = entry.file_name();
            let entry_path = if path.ends_with('/') {
                format!("{}{}", path, name)
            } else {
                format!("{}/{}", path, name)
            };

            if entry.metadata().is_dir() {
                // Recursively delete subdirectory
                delete_directory_recursive(sftp, entry_path).await?;
            } else {
                // Delete file
                sftp.remove_file(entry_path).await.map_err(map_sftp_error)?;
            }
        }

        // Finally, delete the now-empty directory
        sftp.remove_dir(path).await.map_err(map_sftp_error)
    })
}
