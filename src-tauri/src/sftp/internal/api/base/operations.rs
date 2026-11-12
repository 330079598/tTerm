use super::{read_directory_entries, resolve_initial_directory};
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use crate::sftp::internal::connection::{map_sftp_error, release_tab_connections};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::SftpDirectoryListing;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::store;

pub(crate) struct SftpReleaseTabConnectionsInput {
    pub(crate) tab_id: String,
}
pub(crate) struct SftpReleaseTabConnectionsDependencies {
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_release_tab_connections_service(
    dependencies: SftpReleaseTabConnectionsDependencies,
    input: SftpReleaseTabConnectionsInput,
) -> Result<(), String> {
    let SftpReleaseTabConnectionsDependencies { pool } = dependencies;
    let SftpReleaseTabConnectionsInput { tab_id } = input;

    release_tab_connections(&pool, &tab_id).await;
    Ok(())
}

pub(crate) struct SftpListDirectoryInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) path: Option<String>,
}
pub(crate) struct SftpListDirectoryDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_list_directory_service(
    dependencies: SftpListDirectoryDependencies,
    input: SftpListDirectoryInput,
) -> Result<SftpDirectoryListing, String> {
    let SftpListDirectoryDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpListDirectoryInput {
        tab_id,
        connection,
        path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    let requested_path = tterm_core::sftp::internal::paths::sanitize_target_path(path);

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        let current_path = match requested_path {
            Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
            None => resolve_initial_directory(sftp, &plan).await?,
        };

        let entries = read_directory_entries(sftp, &current_path).await?;

        // Save the current path as the last directory. A folder seen as root,
        // such as /root, would only fail the next drawer opened as the user.
        let normalized_path = tterm_core::sftp::internal::paths::normalize_remote_path(&current_path);
        if !plan.sftp_sudo {
            let host = plan.host.as_ref().ok_or("Host is required")?;
            let username = plan.username.as_ref().ok_or("Username is required")?;
            let _ = store::save_last_directory(host, plan.port, username, &normalized_path);
        }

        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: tterm_core::sftp::internal::paths::parent_remote_path(&current_path),
            entries,
        })
    })
}

pub(crate) struct SftpCreateDirectoryInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) path: String,
}
pub(crate) struct SftpCreateDirectoryDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_create_directory_service(
    dependencies: SftpCreateDirectoryDependencies,
    input: SftpCreateDirectoryInput,
) -> Result<(), String> {
    let SftpCreateDirectoryDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpCreateDirectoryInput {
        tab_id,
        connection,
        path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        sftp.create_dir(path).await.map_err(map_sftp_error)
    })
}

pub(crate) struct SftpRenameEntryInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) old_path: String,
    pub(crate) new_path: String,
}
pub(crate) struct SftpRenameEntryDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
}
pub(crate) async fn sftp_rename_entry_service(
    dependencies: SftpRenameEntryDependencies,
    input: SftpRenameEntryInput,
) -> Result<(), String> {
    let SftpRenameEntryDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpRenameEntryInput {
        tab_id,
        connection,
        old_path,
        new_path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        sftp.rename(old_path, new_path).await.map_err(map_sftp_error)
    })
}
