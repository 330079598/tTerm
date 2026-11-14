use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::services::AppServices;
use tterm_core::sftp::internal::api::archive::CompressRequest;
use tterm_core::sftp::internal::api::archive::ExtractRequest;
use tterm_core::sftp::internal::api::archive::RemoteOperationResult;
use tterm_core::sftp::internal::api::archive::operations::SftpCancelRemoteOperationDependencies;
use tterm_core::sftp::internal::api::archive::operations::SftpCancelRemoteOperationInput;
use tterm_core::sftp::internal::api::archive::operations::SftpCompressEntriesDependencies;
use tterm_core::sftp::internal::api::archive::operations::SftpCompressEntriesInput;
use tterm_core::sftp::internal::api::archive::operations::SftpExtractArchiveDependencies;
use tterm_core::sftp::internal::api::archive::operations::SftpExtractArchiveInput;
use tterm_core::sftp::internal::api::archive::operations::sftp_cancel_remote_operation_service;
use tterm_core::sftp::internal::api::archive::operations::sftp_compress_entries_service;
use tterm_core::sftp::internal::api::archive::operations::sftp_extract_archive_service;

/// Packs entries of one folder into a new archive beside them.
#[tauri::command]
pub async fn sftp_compress_entries(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    operation_id: String,
    request: CompressRequest,
    services: State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<RemoteOperationResult, String> {
    sftp_compress_entries_service(
        SftpCompressEntriesDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCompressEntriesInput {
            tab_id,
            connection,
            operation_id,
            request,
        },
    )
    .await
}

/// Unpacks an archive into a folder, which is created when missing.
#[tauri::command]
pub async fn sftp_extract_archive(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    operation_id: String,
    request: ExtractRequest,
    services: State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<RemoteOperationResult, String> {
    sftp_extract_archive_service(
        SftpExtractArchiveDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpExtractArchiveInput {
            tab_id,
            connection,
            operation_id,
            request,
        },
    )
    .await
}

/// Stops a running compress, extract or search.
#[tauri::command]
pub async fn sftp_cancel_remote_operation(
    operation_id: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_cancel_remote_operation_service(
        SftpCancelRemoteOperationDependencies {
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCancelRemoteOperationInput { operation_id },
    )
    .await
}
