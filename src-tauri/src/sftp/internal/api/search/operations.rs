use super::{search, SearchRequest, SearchSummary};
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::{with_cancel, SftpHost};
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::sync::Arc;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::search::SearchResultSink;

pub(super) struct SftpSearchEntriesInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) search_id: String,
    pub(super) request: SearchRequest,
    pub(super) on_event: Arc<dyn SearchResultSink>,
}
pub(super) struct SftpSearchEntriesDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
}
pub(super) async fn sftp_search_entries_service(
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
    with_cancel(cancel_map, &search_id, |cancel| {
        search(host, request, on_event, cancel)
    })
    .await
}
