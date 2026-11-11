use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::delete::batch::{
    preview_delete_entries, run_delete_batch, DeleteBatch,
};
use crate::sftp::internal::api::delete::commands::delete_directory_recursive;
use crate::sftp::internal::api::delete::{
    is_dangerous_delete_path, next_delete_batch_id, DeleteBatchStartResult, DeleteEntryRequest,
    DeleteOptions, DeletePreviewResult,
};
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::connection::{ensure_ssh_plan, SftpContext};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::events::EventSink;
use tterm_core::prompts::PromptBroker;

pub(super) struct SftpPreviewDeleteEntriesInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) entries: Vec<DeleteEntryRequest>,
}
pub(super) struct SftpPreviewDeleteEntriesDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_preview_delete_entries_service(
    dependencies: SftpPreviewDeleteEntriesDependencies,
    input: SftpPreviewDeleteEntriesInput,
) -> Result<DeletePreviewResult, String> {
    let SftpPreviewDeleteEntriesDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpPreviewDeleteEntriesInput {
        tab_id,
        connection,
        entries,
    } = input;

    if entries.is_empty() {
        return Err("No entries selected for deletion".to_string());
    }

    let plan = ensure_ssh_plan(&secrets, connection)?;
    preview_delete_entries(&context, &tab_id, &plan, prompts, &pool, &entries).await
}

pub(super) struct SftpDeleteEntriesInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) entries: Vec<DeleteEntryRequest>,
    pub(super) options: DeleteOptions,
}
pub(super) struct SftpDeleteEntriesDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) events: Arc<dyn EventSink>,
}
pub(super) async fn sftp_delete_entries_service(
    dependencies: SftpDeleteEntriesDependencies,
    input: SftpDeleteEntriesInput,
) -> Result<DeleteBatchStartResult, String> {
    let SftpDeleteEntriesDependencies {
        context,
        prompts,
        secrets,
        pool,
        events,
    } = dependencies;
    let SftpDeleteEntriesInput {
        tab_id,
        connection,
        entries,
        options,
    } = input;

    if entries.is_empty() {
        return Err("No entries selected for deletion".to_string());
    }

    for entry in &entries {
        if is_dangerous_delete_path(&entry.path) {
            return Err(format!("Refusing to delete unsafe path '{}'", entry.path));
        }
    }

    let plan = ensure_ssh_plan(&secrets, connection)?;
    let batch_id = next_delete_batch_id();
    tokio::spawn(run_delete_batch(DeleteBatch {
        app: context,
        events,
        tab_id,
        plan,
        prompts,
        pool,
        batch_id: batch_id.clone(),
        entries,
        options,
    }));

    Ok(DeleteBatchStartResult { batch_id })
}

pub(super) struct SftpDeleteEntryInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: String,
    pub(super) is_dir: bool,
}
pub(super) struct SftpDeleteEntryDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_delete_entry_service(
    dependencies: SftpDeleteEntryDependencies,
    input: SftpDeleteEntryInput,
) -> Result<(), String> {
    let SftpDeleteEntryDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpDeleteEntryInput {
        tab_id,
        connection,
        path,
        is_dir,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        if is_dir {
            delete_directory_recursive(sftp, path).await
        } else {
            sftp.remove_file(path).await.map_err(map_sftp_error)
        }
    })
}
