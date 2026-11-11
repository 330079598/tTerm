use super::{read_directory_entries, resolve_initial_directory};
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{ensure_ssh_plan, SftpContext};
use crate::sftp::internal::connection::{map_sftp_error, release_tab_connections};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::SftpDirectoryListing;
use crate::sftp::store;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;

pub(super) struct SftpReleaseTabConnectionsInput {
    pub(super) tab_id: String,
}
pub(super) struct SftpReleaseTabConnectionsDependencies {
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_release_tab_connections_service(
    dependencies: SftpReleaseTabConnectionsDependencies,
    input: SftpReleaseTabConnectionsInput,
) -> Result<(), String> {
    let SftpReleaseTabConnectionsDependencies { pool } = dependencies;
    let SftpReleaseTabConnectionsInput { tab_id } = input;

    let pool_state = &pool;
    release_tab_connections(pool_state, &tab_id).await;
    Ok(())
}

pub(super) struct SftpListDirectoryInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: Option<String>,
}
pub(super) struct SftpListDirectoryDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_list_directory_service(
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

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;
    let plan = ensure_ssh_plan(secret_state, connection)?;

    let requested_path = crate::sftp::internal::paths::sanitize_target_path(path);

    with_sftp!(&app, &tab_id, &plan, prompt_state.clone(), pool_state, sftp => {
        let current_path = match requested_path {
            Some(path) => sftp.canonicalize(path).await.map_err(map_sftp_error)?,
            None => resolve_initial_directory(sftp, &plan).await?,
        };

        let entries = read_directory_entries(sftp, &current_path).await?;

        // Save the current path as the last directory. A folder seen as root,
        // such as /root, would only fail the next drawer opened as the user.
        let normalized_path = crate::sftp::internal::paths::normalize_remote_path(&current_path);
        if !plan.sftp_sudo {
            let host = plan.host.as_ref().ok_or("Host is required")?;
            let username = plan.username.as_ref().ok_or("Username is required")?;
            let _ = store::save_last_directory(host, plan.port, username, &normalized_path);
        }

        Ok(SftpDirectoryListing {
            current_path: normalized_path,
            parent_path: crate::sftp::internal::paths::parent_remote_path(&current_path),
            entries,
        })
    })
}

pub(super) struct SftpCreateDirectoryInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: String,
}
pub(super) struct SftpCreateDirectoryDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_create_directory_service(
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

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;
    let plan = ensure_ssh_plan(secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.clone(), pool_state, sftp => {
        sftp.create_dir(path).await.map_err(map_sftp_error)
    })
}

pub(super) struct SftpRenameEntryInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) old_path: String,
    pub(super) new_path: String,
}
pub(super) struct SftpRenameEntryDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_rename_entry_service(
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

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;

    let plan = ensure_ssh_plan(secret_state, connection)?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.clone(), pool_state, sftp => {
        sftp.rename(old_path, new_path).await.map_err(map_sftp_error)
    })
}
