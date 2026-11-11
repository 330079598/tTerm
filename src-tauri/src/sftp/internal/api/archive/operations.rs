use super::{compress, extract, CompressRequest, ExtractRequest, RemoteOperationResult};
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::{with_cancel, SftpHost};
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;

pub(super) struct SftpCompressEntriesInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) operation_id: String,
    pub(super) request: CompressRequest,
}
pub(super) struct SftpCompressEntriesDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
}
pub(super) async fn sftp_compress_entries_service(
    dependencies: SftpCompressEntriesDependencies,
    input: SftpCompressEntriesInput,
) -> Result<RemoteOperationResult, String> {
    let SftpCompressEntriesDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
    } = dependencies;
    let SftpCompressEntriesInput {
        tab_id,
        connection,
        operation_id,
        request,
    } = input;

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;
    let cancel_map = &cancel;

    let host = SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: prompt_state.clone(),
        secret_state,
        pool: pool_state,
    };
    with_cancel(cancel_map, &operation_id, |cancel| {
        compress(host, request, cancel)
    })
    .await
}

pub(super) struct SftpExtractArchiveInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) operation_id: String,
    pub(super) request: ExtractRequest,
}
pub(super) struct SftpExtractArchiveDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
}
pub(super) async fn sftp_extract_archive_service(
    dependencies: SftpExtractArchiveDependencies,
    input: SftpExtractArchiveInput,
) -> Result<RemoteOperationResult, String> {
    let SftpExtractArchiveDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
    } = dependencies;
    let SftpExtractArchiveInput {
        tab_id,
        connection,
        operation_id,
        request,
    } = input;

    let app = context;
    let prompt_state = prompts.clone();
    let secret_state = &secrets;
    let pool_state = &pool;
    let cancel_map = &cancel;

    let host = SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: prompt_state.clone(),
        secret_state,
        pool: pool_state,
    };
    with_cancel(cancel_map, &operation_id, |cancel| {
        extract(host, request, cancel)
    })
    .await
}

pub(super) struct SftpCancelRemoteOperationInput {
    pub(super) operation_id: String,
}
pub(super) struct SftpCancelRemoteOperationDependencies {
    pub(super) cancel: TransferCancelMap,
}
pub(super) async fn sftp_cancel_remote_operation_service(
    dependencies: SftpCancelRemoteOperationDependencies,
    input: SftpCancelRemoteOperationInput,
) -> Result<(), String> {
    let SftpCancelRemoteOperationDependencies { cancel } = dependencies;
    let SftpCancelRemoteOperationInput { operation_id } = input;

    let cancel_map = &cancel;
    if let Some(sender) = cancel_map.read().await.get(&operation_id) {
        let _ = sender.send(true);
    }
    Ok(())
}
