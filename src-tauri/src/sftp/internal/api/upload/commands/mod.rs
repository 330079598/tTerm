mod operations;
use crate::services::AppServices;
use crate::sftp::internal::api::conflict::ConflictReport;
use crate::sftp::internal::api::upload::UploadBatchResult;
use operations::{
    SftpCancelUploadDependencies, SftpCancelUploadInput, SftpCheckUploadConflictsDependencies,
    SftpCheckUploadConflictsInput, SftpUploadFileDependencies, SftpUploadFileInput,
    SftpUploadPathsDependencies, SftpUploadPathsInput, sftp_cancel_upload_service,
    sftp_check_upload_conflicts_service, sftp_upload_file_service, sftp_upload_paths_service,
};
use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::sftp::conflict::ConflictPolicy;

/// Pre-flight for `sftp_upload_paths`: which planned files already exist on
/// the remote. Each destination directory is listed once rather than every
/// file being stat'ed, so a large folder costs as many round trips as the
/// upload spends creating its directories.
#[tauri::command]
pub async fn sftp_check_upload_conflicts(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ConflictReport, String> {
    sftp_check_upload_conflicts_service(
        SftpCheckUploadConflictsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpCheckUploadConflictsInput {
            tab_id,
            connection,
            local_paths,
            remote_base_path,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_upload_file(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_path: String,
    remote_path: String,
    transfer_id: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_upload_file_service(
        SftpUploadFileDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpUploadFileInput {
            tab_id,
            connection,
            local_path,
            remote_path,
            transfer_id,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_upload_paths(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    skip_existing: Option<bool>,
    conflict_policy: Option<ConflictPolicy>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<UploadBatchResult, String> {
    sftp_upload_paths_service(
        SftpUploadPathsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpUploadPathsInput {
            tab_id,
            connection,
            local_paths,
            remote_base_path,
            skip_existing,
            conflict_policy,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_cancel_upload(
    transfer_id: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_cancel_upload_service(
        SftpCancelUploadDependencies {
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCancelUploadInput { transfer_id },
    )
    .await
}
