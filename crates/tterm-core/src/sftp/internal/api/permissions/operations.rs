use crate::core::session::types::PtyConnectionOptions;
use crate::prompts::PromptBroker;
use crate::sftp::internal::api::permissions::set_permissions;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;

pub struct SftpSetPermissionsInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub paths: Vec<String>,
    pub mode: u32,
}
pub struct SftpSetPermissionsDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_set_permissions_service(
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
