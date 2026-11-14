use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::services::AppServices;
use tterm_core::sftp::internal::api::edit::SftpEditBaseline;
use tterm_core::sftp::internal::api::edit::SftpEditableFile;
use tterm_core::sftp::internal::api::edit::SftpSaveEditedFileResult;
use tterm_core::sftp::internal::api::edit::operations::SftpOpenFileForEditDependencies;
use tterm_core::sftp::internal::api::edit::operations::SftpOpenFileForEditInput;
use tterm_core::sftp::internal::api::edit::operations::SftpSaveEditedFileDependencies;
use tterm_core::sftp::internal::api::edit::operations::SftpSaveEditedFileInput;
use tterm_core::sftp::internal::api::edit::operations::sftp_open_file_for_edit_service;
use tterm_core::sftp::internal::api::edit::operations::sftp_save_edited_file_service;

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
    services: State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
