mod operations;
use crate::services::AppServices;
use crate::{
    sftp::internal::api::delete::DeleteBatchStartResult,
    sftp::internal::api::delete::DeleteEntryRequest, sftp::internal::api::delete::DeleteOptions,
    sftp::internal::api::delete::DeletePreviewResult,
};
use operations::{
    SftpDeleteEntriesDependencies, SftpDeleteEntriesInput, SftpDeleteEntryDependencies,
    SftpDeleteEntryInput, SftpPreviewDeleteEntriesDependencies, SftpPreviewDeleteEntriesInput,
    sftp_delete_entries_service, sftp_delete_entry_service, sftp_preview_delete_entries_service,
};
use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
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
