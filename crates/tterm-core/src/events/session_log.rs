use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalLogStatus {
    /// Sessions are logged unless their connection or tab says otherwise.
    pub enabled: bool,
    pub directory: String,
    pub active_sessions: usize,
    pub recording_tab_ids: Vec<String>,
    pub total_size_bytes: u64,
    pub last_error: Option<String>,
}
