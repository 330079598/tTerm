use serde::Serialize;

/// The existing deletion method reported to the client.
#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DeleteMethod {
    /// Deletes entries recursively through SFTP operations.
    Sftp,
    /// Deletes entries by executing a command on the remote host.
    Command,
}

/// The selected entry names and bounded deletion statistics.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchStartEvent {
    /// Correlates the start, progress and completion of the same deletion run.
    pub batch_id: String,
    /// Names of the selected top-level entries, rather than full remote paths.
    pub entries: Vec<String>,
    pub method: DeleteMethod,
    /// Directories counted by the bounded pre-scan, including selected roots.
    pub total_directories: usize,
    /// Files and directories counted by the bounded pre-scan.
    pub total_entries: usize,
    pub total_files: usize,
    /// The pre-scan reached its limit, so the totals may be incomplete.
    pub total_truncated: bool,
}

/// Sampled deletion counters and the current path.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProgressEvent {
    pub batch_id: String,
    /// The sampled remote path, or an empty string for the final snapshot.
    pub current_path: String,
    /// Command deletion reports pre-scan counts after a successful exit.
    pub deleted_directories: usize,
    /// Command deletion reports pre-scan counts after a successful exit.
    pub deleted_files: usize,
    /// Failed deletion operations counted so far.
    pub failed: usize,
    pub method: DeleteMethod,
    /// Directory count from the bounded pre-scan, rather than a live total.
    pub total_directories: usize,
    /// File and directory count from the bounded pre-scan.
    pub total_entries: usize,
    pub total_files: usize,
    /// The pre-scan reached its limit, so the totals may be incomplete.
    pub total_truncated: bool,
}

/// The final deletion counters and error, if any.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchCompleteEvent {
    pub batch_id: String,
    /// Retained for the client protocol; deletion currently has no cancellation.
    pub cancelled: bool,
    /// Command deletion reports pre-scan counts after a successful exit.
    pub deleted_directories: usize,
    /// Command deletion reports pre-scan counts after a successful exit.
    pub deleted_files: usize,
    pub error: Option<String>,
    /// Failed deletion operations, including a batch-level failure if present.
    pub failed: usize,
    pub method: DeleteMethod,
    /// Directory count from the bounded pre-scan, rather than a live total.
    pub total_directories: usize,
    /// File and directory count from the bounded pre-scan.
    pub total_entries: usize,
    pub total_files: usize,
    /// The pre-scan reached its limit, so the totals may be incomplete.
    pub total_truncated: bool,
}
