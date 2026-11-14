use crate::sftp::internal::api::permissions::set_permissions;
use std::sync::Arc;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use tterm_core::sftp::internal::types::SftpConnectionPool;
use tterm_core::ssh::secret_store::types::SecretStoreState;

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
