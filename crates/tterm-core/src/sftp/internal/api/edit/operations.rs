use crate::core::session::types::PtyConnectionOptions;
use crate::prompts::PromptBroker;
use crate::sftp::internal::api::edit::SftpEditBaseline;
use crate::sftp::internal::api::edit::SftpEditableFile;
use crate::sftp::internal::api::edit::SftpSaveEditedFileResult;
use crate::sftp::internal::api::edit::load_editable_file;
use crate::sftp::internal::api::edit::save_editable_file;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;

pub struct SftpOpenFileForEditInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub path: String,
}
pub struct SftpOpenFileForEditDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_open_file_for_edit_service(
    dependencies: SftpOpenFileForEditDependencies,
    input: SftpOpenFileForEditInput,
) -> Result<SftpEditableFile, String> {
    let SftpOpenFileForEditDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpOpenFileForEditInput {
        tab_id,
        connection,
        path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        load_editable_file(sftp, &path).await
    })
}

pub struct SftpSaveEditedFileInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub path: String,
    pub content: String,
    pub baseline: SftpEditBaseline,
}
pub struct SftpSaveEditedFileDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_save_edited_file_service(
    dependencies: SftpSaveEditedFileDependencies,
    input: SftpSaveEditedFileInput,
) -> Result<SftpSaveEditedFileResult, String> {
    let SftpSaveEditedFileDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpSaveEditedFileInput {
        tab_id,
        connection,
        path,
        content,
        baseline,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        save_editable_file(sftp, &path, &content, &baseline).await
    })
}
