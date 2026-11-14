use crate::core::session::types::PtyConnectionOptions;
use crate::prompts::PromptBroker;
use crate::sftp::internal::api::SftpHost;
use crate::sftp::internal::api::archive::CompressRequest;
use crate::sftp::internal::api::archive::ExtractRequest;
use crate::sftp::internal::api::archive::RemoteOperationResult;
use crate::sftp::internal::api::archive::compress;
use crate::sftp::internal::api::archive::extract;
use crate::sftp::internal::api::with_cancel;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;

pub struct SftpCompressEntriesInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub operation_id: String,
    pub request: CompressRequest,
}
pub struct SftpCompressEntriesDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub cancel: TransferCancelMap,
}
pub async fn sftp_compress_entries_service(
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

pub struct SftpExtractArchiveInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub operation_id: String,
    pub request: ExtractRequest,
}
pub struct SftpExtractArchiveDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub cancel: TransferCancelMap,
}
pub async fn sftp_extract_archive_service(
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

pub struct SftpCancelRemoteOperationInput {
    pub operation_id: String,
}
pub struct SftpCancelRemoteOperationDependencies {
    pub cancel: TransferCancelMap,
}
pub async fn sftp_cancel_remote_operation_service(
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
