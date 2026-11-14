use crate::sftp::internal::api::conflict::ConflictReport;
use crate::{
    sftp::internal::api::download::operations::SftpCheckDownloadConflictsDependencies,
    sftp::internal::api::download::operations::SftpCheckDownloadConflictsInput,
    sftp::internal::api::download::operations::SftpDownloadFileDependencies,
    sftp::internal::api::download::operations::SftpDownloadFileInput,
    sftp::internal::api::download::operations::SftpDownloadPathsDependencies,
    sftp::internal::api::download::operations::SftpDownloadPathsInput,
    sftp::internal::api::download::operations::get_file_size_service,
    sftp::internal::api::download::operations::sftp_check_download_conflicts_service,
    sftp::internal::api::download::operations::sftp_download_file_service,
    sftp::internal::api::download::operations::sftp_download_paths_service,
};
use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::sftp::conflict::ConflictPolicy;

#[tauri::command]
pub async fn sftp_download_file(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_path: String,
    local_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_download_file_service(
        SftpDownloadFileDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpDownloadFileInput {
            tab_id,
            connection,
            transfer_id,
            remote_path,
            local_path,
        },
    )
    .await
}

/// Downloads remote files and folders, as one transfer, into
/// `local_parent_path`.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_download_paths(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    transfer_id: String,
    remote_paths: Vec<String>,
    local_parent_path: String,
    skip_existing: Option<bool>,
    conflict_policy: Option<ConflictPolicy>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_download_paths_service(
        SftpDownloadPathsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpDownloadPathsInput {
            tab_id,
            connection,
            transfer_id,
            remote_paths,
            local_parent_path,
            skip_existing,
            conflict_policy,
        },
    )
    .await
}

/// Pre-flight for `sftp_download_paths`: which of the files to download
/// already exist locally. Nothing can collide unless one of the remote roots
/// already exists locally, so the remote trees are only walked when one does.
#[tauri::command]
pub async fn sftp_check_download_conflicts(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    remote_paths: Vec<String>,
    local_parent_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ConflictReport, String> {
    sftp_check_download_conflicts_service(
        SftpCheckDownloadConflictsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpCheckDownloadConflictsInput {
            tab_id,
            connection,
            remote_paths,
            local_parent_path,
        },
    )
    .await
}

#[tauri::command]
pub async fn get_file_size(local_path: String) -> Result<u64, String> {
    get_file_size_service(local_path).await
}
