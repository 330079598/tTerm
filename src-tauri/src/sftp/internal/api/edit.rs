use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::SecretStoreState;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_EDIT_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpEditableFile {
    content: String,
    file_name: String,
    modified_at: Option<i64>,
    path: String,
    size: u64,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpEditBaseline {
    modified_at: Option<i64>,
    size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpSaveEditedFileResult {
    modified_at: Option<i64>,
    size: u64,
}

fn metadata_modified_at(metadata: &russh_sftp::protocol::FileAttributes) -> Option<i64> {
    metadata.mtime.map(|value| value as i64 * 1000)
}

fn file_name_from_remote_path(path: &str) -> Result<String, String> {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .map(ToString::to_string)
        .ok_or_else(|| format!("Failed to determine file name for remote path '{path}'"))
}

fn temp_remote_path(path: &str) -> Result<String, String> {
    let parent = crate::sftp::internal::paths::parent_remote_path(path)
        .ok_or_else(|| format!("Failed to determine parent directory for remote path '{path}'"))?;
    let file_name = file_name_from_remote_path(path)?;
    Ok(crate::sftp::internal::paths::join_remote_path(
        &parent,
        &format!(".{file_name}.tterm-save-{}", uuid::Uuid::new_v4()),
    ))
}

async fn load_editable_file(sftp: &SftpSession, path: &str) -> Result<SftpEditableFile, String> {
    let mut remote_file = sftp.open(path).await.map_err(map_sftp_error)?;
    let metadata = remote_file.metadata().await.map_err(map_sftp_error)?;

    if metadata.is_dir() {
        return Err("Directories cannot be edited".to_string());
    }

    let size = metadata.size.unwrap_or(0);
    if size > MAX_EDIT_FILE_BYTES {
        return Err(format!(
            "Remote file is too large to edit in tTerm ({} bytes, limit {} bytes)",
            size, MAX_EDIT_FILE_BYTES
        ));
    }

    let mut bytes = Vec::with_capacity(size as usize);
    remote_file
        .read_to_end(&mut bytes)
        .await
        .map_err(|err| format!("Failed to read remote file '{path}': {err}"))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| "Only UTF-8 text files can be edited in tTerm".to_string())?;

    Ok(SftpEditableFile {
        content,
        file_name: file_name_from_remote_path(path)?,
        modified_at: metadata_modified_at(&metadata),
        path: crate::sftp::internal::paths::normalize_remote_path(path),
        size,
    })
}

async fn save_editable_file(
    sftp: &SftpSession,
    path: &str,
    content: &str,
    baseline: &SftpEditBaseline,
) -> Result<SftpSaveEditedFileResult, String> {
    let metadata = sftp.metadata(path).await.map_err(map_sftp_error)?;
    let current_size = metadata.size.unwrap_or(0);
    let current_modified_at = metadata_modified_at(&metadata);

    if current_size != baseline.size || current_modified_at != baseline.modified_at {
        return Err(
            "Remote file changed after it was opened. Reload it before saving.".to_string(),
        );
    }

    let temp_path = temp_remote_path(path)?;
    let write_result = async {
        let mut remote_file = sftp.create(&temp_path).await.map_err(map_sftp_error)?;
        remote_file
            .write_all(content.as_bytes())
            .await
            .map_err(|err| format!("Failed to write remote file '{temp_path}': {err}"))?;
        remote_file
            .flush()
            .await
            .map_err(|err| format!("Failed to flush remote file '{temp_path}': {err}"))?;
        remote_file.sync_all().await.map_err(map_sftp_error)?;
        Ok::<(), String>(())
    }
    .await;

    if let Err(error) = write_result {
        let _ = sftp.remove_file(&temp_path).await;
        return Err(error);
    }

    if let Err(error) = sftp.rename(&temp_path, path).await {
        let replace_error = map_sftp_error(error);
        let metadata = sftp.metadata(path).await.map_err(map_sftp_error)?;
        let current_size = metadata.size.unwrap_or(0);
        let current_modified_at = metadata_modified_at(&metadata);

        if current_size != baseline.size || current_modified_at != baseline.modified_at {
            let _ = sftp.remove_file(&temp_path).await;
            return Err(
                "Remote file changed while saving. Reload it before trying again.".to_string(),
            );
        }

        let direct_write_result = async {
            let mut remote_file = sftp.create(path).await.map_err(map_sftp_error)?;
            remote_file
                .write_all(content.as_bytes())
                .await
                .map_err(|err| format!("Failed to write remote file '{path}': {err}"))?;
            remote_file
                .flush()
                .await
                .map_err(|err| format!("Failed to flush remote file '{path}': {err}"))?;
            remote_file.sync_all().await.map_err(map_sftp_error)?;
            Ok::<(), String>(())
        }
        .await;

        let _ = sftp.remove_file(&temp_path).await;

        if let Err(error) = direct_write_result {
            return Err(format!(
                "Failed to replace remote file '{}' with rename ({}), then direct overwrite failed: {}",
                path, replace_error, error
            ));
        }
    }

    let next_metadata = sftp.metadata(path).await.map_err(map_sftp_error)?;
    Ok(SftpSaveEditedFileResult {
        modified_at: metadata_modified_at(&next_metadata),
        size: next_metadata.size.unwrap_or(content.len() as u64),
    })
}

#[tauri::command]
pub async fn sftp_open_file_for_edit(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<SftpEditableFile, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        load_editable_file(sftp, &path).await
    })
}

#[tauri::command]
pub async fn sftp_save_edited_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    content: String,
    baseline: SftpEditBaseline,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<SftpSaveEditedFileResult, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        save_editable_file(sftp, &path, &content, &baseline).await
    })
}
