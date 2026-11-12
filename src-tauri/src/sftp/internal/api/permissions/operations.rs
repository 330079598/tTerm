use super::set_permissions;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;

pub(crate) struct SftpSetPermissionsInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) paths: Vec<String>,
    pub(crate) mode: u32,
}
pub(crate) struct SftpSetPermissionsDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_set_permissions_service(
    dependencies: SftpSetPermissionsDependencies,
    input: SftpSetPermissionsInput,
) -> Result<(), String> {
    let SftpSetPermissionsDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpSetPermissionsInput {
        tab_id,
        connection,
        paths,
        mode,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        set_permissions(sftp, &paths, mode).await
    })
}
