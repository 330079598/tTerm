use crate::services::AppServices;
use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::sftp::internal::api::delete::DeleteBatchStartResult;
use tterm_core::sftp::internal::api::delete::DeleteEntryRequest;
use tterm_core::sftp::internal::api::delete::DeleteOptions;
use tterm_core::sftp::internal::api::delete::DeletePreviewResult;
use tterm_core::sftp::internal::api::delete::operations::SftpDeleteEntriesDependencies;
use tterm_core::sftp::internal::api::delete::operations::SftpDeleteEntriesInput;
use tterm_core::sftp::internal::api::delete::operations::SftpDeleteEntryDependencies;
use tterm_core::sftp::internal::api::delete::operations::SftpDeleteEntryInput;
use tterm_core::sftp::internal::api::delete::operations::SftpPreviewDeleteEntriesDependencies;
use tterm_core::sftp::internal::api::delete::operations::SftpPreviewDeleteEntriesInput;
use tterm_core::sftp::internal::api::delete::operations::sftp_delete_entries_service;
use tterm_core::sftp::internal::api::delete::operations::sftp_delete_entry_service;
use tterm_core::sftp::internal::api::delete::operations::sftp_preview_delete_entries_service;
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
