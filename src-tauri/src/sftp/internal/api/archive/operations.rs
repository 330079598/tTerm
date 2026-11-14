use crate::{sftp::internal::api::SftpHost, sftp::internal::api::with_cancel};
use crate::{
    sftp::internal::api::archive::CompressRequest, sftp::internal::api::archive::ExtractRequest,
    sftp::internal::api::archive::RemoteOperationResult, sftp::internal::api::archive::compress,
    sftp::internal::api::archive::extract,
};
use std::sync::Arc;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::internal::connection::SftpContext;
use tterm_core::sftp::internal::types::SftpConnectionPool;
use tterm_core::sftp::internal::types::TransferCancelMap;
use tterm_core::ssh::secret_store::types::SecretStoreState;

pub(crate) struct SftpCompressEntriesInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) operation_id: String,
    pub(crate) request: CompressRequest,
}
pub(crate) struct SftpCompressEntriesDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
    pub(crate) cancel: TransferCancelMap,
}
pub(crate) async fn sftp_compress_entries_service(
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

    let host = SftpHost {
        app: &context,
        tab_id: &tab_id,
        connection,
        prompts,
        secret_state: &secrets,
        pool: &pool,
    };
    with_cancel(&cancel, &operation_id, |cancel| {
        compress(host, request, cancel)
    })
    .await
}

pub(crate) struct SftpExtractArchiveInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) operation_id: String,
    pub(crate) request: ExtractRequest,
}
pub(crate) struct SftpExtractArchiveDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
    pub(crate) cancel: TransferCancelMap,
}
pub(crate) async fn sftp_extract_archive_service(
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

    let host = SftpHost {
        app: &context,
        tab_id: &tab_id,
        connection,
        prompts,
        secret_state: &secrets,
        pool: &pool,
    };
    with_cancel(&cancel, &operation_id, |cancel| {
        extract(host, request, cancel)
    })
    .await
}

pub(crate) struct SftpCancelRemoteOperationInput {
    pub(crate) operation_id: String,
}
pub(crate) struct SftpCancelRemoteOperationDependencies {
    pub(crate) cancel: TransferCancelMap,
}
pub(crate) async fn sftp_cancel_remote_operation_service(
    dependencies: SftpCancelRemoteOperationDependencies,
    input: SftpCancelRemoteOperationInput,
) -> Result<(), String> {
    let SftpCancelRemoteOperationDependencies { cancel } = dependencies;
    let SftpCancelRemoteOperationInput { operation_id } = input;

    if let Some(sender) = cancel.read().await.get(&operation_id) {
        let _ = sender.send(true);
    }
    Ok(())
}
