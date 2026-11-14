use crate::core::session::types::PtyConnectionOptions;
use crate::events::EventSink;
use crate::prompts::PromptBroker;
use crate::sftp::internal::api::delete::DeleteBatchStartResult;
use crate::sftp::internal::api::delete::DeleteEntryRequest;
use crate::sftp::internal::api::delete::DeleteOptions;
use crate::sftp::internal::api::delete::DeletePreviewResult;
use crate::sftp::internal::api::delete::batch::DeleteBatch;
use crate::sftp::internal::api::delete::batch::preview_delete_entries;
use crate::sftp::internal::api::delete::batch::run_delete_batch;
use crate::sftp::internal::api::delete::is_dangerous_delete_path;
use crate::sftp::internal::api::delete::next_delete_batch_id;
use crate::sftp::internal::api::delete::recursive::delete_directory_recursive;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;

pub struct SftpPreviewDeleteEntriesInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub entries: Vec<DeleteEntryRequest>,
}
pub struct SftpPreviewDeleteEntriesDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_preview_delete_entries_service(
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

pub struct SftpDeleteEntriesInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub entries: Vec<DeleteEntryRequest>,
    pub options: DeleteOptions,
}
pub struct SftpDeleteEntriesDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub events: Arc<dyn EventSink>,
}
pub async fn sftp_delete_entries_service(
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

pub struct SftpDeleteEntryInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub path: String,
    pub is_dir: bool,
}
pub struct SftpDeleteEntryDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_delete_entry_service(
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
