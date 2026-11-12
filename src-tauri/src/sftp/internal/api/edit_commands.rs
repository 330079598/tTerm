use super::operations::{
    SftpOpenFileForEditDependencies, SftpOpenFileForEditInput, SftpSaveEditedFileDependencies,
    SftpSaveEditedFileInput, sftp_open_file_for_edit_service, sftp_save_edited_file_service,
};
use super::{SftpEditBaseline, SftpEditableFile, SftpSaveEditedFileResult};
use crate::core::session::types::PtyConnectionOptions;
use crate::services::AppServices;
use tauri::State;

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
