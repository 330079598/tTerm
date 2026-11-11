use super::{
    load_editable_file, save_editable_file, SftpEditBaseline, SftpEditableFile,
    SftpSaveEditedFileResult,
};
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{ensure_ssh_plan, SftpContext};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;

pub(super) struct SftpOpenFileForEditInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: String,
}
pub(super) struct SftpOpenFileForEditDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_open_file_for_edit_service(
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

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;
    let plan = ensure_ssh_plan(secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.clone(), pool_state, sftp => {
        load_editable_file(sftp, &path).await
    })
}

pub(super) struct SftpSaveEditedFileInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: String,
    pub(super) content: String,
    pub(super) baseline: SftpEditBaseline,
}
pub(super) struct SftpSaveEditedFileDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_save_edited_file_service(
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

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;

    let plan = ensure_ssh_plan(secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.clone(), pool_state, sftp => {
        save_editable_file(sftp, &path, &content, &baseline).await
    })
}
