use super::set_permissions;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;

pub(super) struct SftpSetPermissionsInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) paths: Vec<String>,
    pub(super) mode: u32,
}
pub(super) struct SftpSetPermissionsDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_set_permissions_service(
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
