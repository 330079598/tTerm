use crate::{sftp::internal::api::SftpHost, sftp::internal::api::with_cancel};
use crate::{
    sftp::internal::api::search::SearchRequest, sftp::internal::api::search::SearchSummary,
    sftp::internal::api::search::search,
};
use std::sync::Arc;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::internal::connection::SftpContext;
use tterm_core::sftp::internal::types::SftpConnectionPool;
use tterm_core::sftp::internal::types::TransferCancelMap;
use tterm_core::sftp::search::SearchResultSink;
use tterm_core::ssh::secret_store::types::SecretStoreState;

pub(crate) struct SftpSearchEntriesInput {
    pub(crate) tab_id: String,
    pub(crate) connection: Option<PtyConnectionOptions>,
    pub(crate) search_id: String,
    pub(crate) request: SearchRequest,
    pub(crate) on_event: Arc<dyn SearchResultSink>,
}
pub(crate) struct SftpSearchEntriesDependencies {
    pub(crate) context: SftpContext,
    pub(crate) prompts: Arc<PromptBroker>,
    pub(crate) secrets: SecretStoreState,
    pub(crate) pool: SftpConnectionPool,
    pub(crate) cancel: TransferCancelMap,
}
pub(crate) async fn sftp_search_entries_service(
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
