mod operations;
use super::{DeleteBatchStartResult, DeleteEntryRequest, DeleteOptions, DeletePreviewResult};
use crate::core::session::types::PtyConnectionOptions;
use crate::services::AppServices;
use crate::sftp::internal::connection::map_sftp_error;
use operations::{
    SftpDeleteEntriesDependencies, SftpDeleteEntriesInput, SftpDeleteEntryDependencies,
    SftpDeleteEntryInput, SftpPreviewDeleteEntriesDependencies, SftpPreviewDeleteEntriesInput,
    sftp_delete_entries_service, sftp_delete_entry_service, sftp_preview_delete_entries_service,
};
use russh_sftp::client::SftpSession;
use std::future::Future;
use std::pin::Pin;
use tauri::State;
#[tauri::command]
pub async fn sftp_preview_delete_entries(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    entries: Vec<DeleteEntryRequest>,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<DeletePreviewResult, String> {
    sftp_preview_delete_entries_service(
        SftpPreviewDeleteEntriesDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpPreviewDeleteEntriesInput {
            tab_id,
            connection,
            entries,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_delete_entries(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    entries: Vec<DeleteEntryRequest>,
    options: DeleteOptions,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<DeleteBatchStartResult, String> {
    sftp_delete_entries_service(
        SftpDeleteEntriesDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            events: services.shared.events.clone(),
        },
        SftpDeleteEntriesInput {
            tab_id,
            connection,
            entries,
            options,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_delete_entry(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    is_dir: bool,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_delete_entry_service(
        SftpDeleteEntryDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpDeleteEntryInput {
            tab_id,
            connection,
            path,
            is_dir,
        },
    )
    .await
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
