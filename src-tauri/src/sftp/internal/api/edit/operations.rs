use crate::{
    sftp::internal::api::edit::SftpEditBaseline, sftp::internal::api::edit::SftpEditableFile,
    sftp::internal::api::edit::SftpSaveEditedFileResult,
    sftp::internal::api::edit::load_editable_file, sftp::internal::api::edit::save_editable_file,
};
use std::sync::Arc;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use tterm_core::sftp::internal::types::SftpConnectionPool;
use tterm_core::ssh::secret_store::types::SecretStoreState;

pub(crate) struct SftpOpenFileForEditInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) path: String,
}
pub(crate) struct SftpOpenFileForEditDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_open_file_for_edit_service(
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

pub(crate) struct SftpSaveEditedFileInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) baseline: SftpEditBaseline,
}
pub(crate) struct SftpSaveEditedFileDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_save_edited_file_service(
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
