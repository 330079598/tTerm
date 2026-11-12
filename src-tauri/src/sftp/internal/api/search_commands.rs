use super::operations::{
    SftpSearchEntriesDependencies, SftpSearchEntriesInput, sftp_search_entries_service,
};
use super::{SearchRequest, SearchSummary};
use crate::core::session::types::PtyConnectionOptions;
use crate::event_sink::search::TauriSearchResultSink;
use std::sync::Arc;
use tauri::State;
use tauri::ipc::Channel;
use tterm_core::sftp::search::SearchEvent;

/// Searches `request.root` and the folders under it for names matching the
/// query, sending matches on `on_event` as they are found. Resolves once the
/// search ends; `sftp_cancel_remote_operation` with `search_id` stops it.
#[tauri::command]
pub async fn sftp_search_entries(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    search_id: String,
    request: SearchRequest,
    on_event: Channel<SearchEvent>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<SearchSummary, String> {
    sftp_search_entries_service(
        SftpSearchEntriesDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpSearchEntriesInput {
            tab_id,
            connection,
            search_id,
            request,
            on_event: Arc::new(TauriSearchResultSink::new(on_event)),
        },
    )
    .await
}
