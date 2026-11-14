use crate::services::AppServices;
use crate::{
    sftp::internal::api::edit::SftpEditBaseline, sftp::internal::api::edit::SftpEditableFile,
    sftp::internal::api::edit::SftpSaveEditedFileResult,
};
use crate::{
    sftp::internal::api::edit::operations::SftpOpenFileForEditDependencies,
    sftp::internal::api::edit::operations::SftpOpenFileForEditInput,
    sftp::internal::api::edit::operations::SftpSaveEditedFileDependencies,
    sftp::internal::api::edit::operations::SftpSaveEditedFileInput,
    sftp::internal::api::edit::operations::sftp_open_file_for_edit_service,
    sftp::internal::api::edit::operations::sftp_save_edited_file_service,
};
use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;

#[tauri::command]
pub async fn sftp_open_file_for_edit(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<SftpEditableFile, String> {
    sftp_open_file_for_edit_service(
        SftpOpenFileForEditDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpOpenFileForEditInput {
            tab_id,
            connection,
            path,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_save_edited_file(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    content: String,
    baseline: SftpEditBaseline,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SftpSaveEditedFileResult, String> {
    sftp_save_edited_file_service(
        SftpSaveEditedFileDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpSaveEditedFileInput {
            tab_id,
            connection,
            path,
            content,
            baseline,
        },
    )
    .await
}
