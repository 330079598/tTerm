use crate::core::session::types::PtyConnectionOptions;
use crate::prompts::PromptBroker;
use crate::sftp::internal::api::SftpHost;
use crate::sftp::internal::api::search::SearchRequest;
use crate::sftp::internal::api::search::SearchSummary;
use crate::sftp::internal::api::search::search;
use crate::sftp::internal::api::with_cancel;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::sftp::search::SearchResultSink;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;

pub struct SftpSearchEntriesInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub search_id: String,
    pub request: SearchRequest,
    pub on_event: Arc<dyn SearchResultSink>,
}
pub struct SftpSearchEntriesDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub cancel: TransferCancelMap,
}
pub async fn sftp_search_entries_service(
    dependencies: SftpSearchEntriesDependencies,
    input: SftpSearchEntriesInput,
) -> Result<SearchSummary, String> {
    let SftpSearchEntriesDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
    } = dependencies;
    let SftpSearchEntriesInput {
        tab_id,
        connection,
        search_id,
        request,
        on_event,
    } = input;

    let host = SftpHost {
        app: &context,
        tab_id: &tab_id,
        connection,
        prompts,
        secret_state: &secrets,
        pool: &pool,
    };
    with_cancel(&cancel, &search_id, |cancel| {
        search(host, request, on_event, cancel)
    })
    .await
}
