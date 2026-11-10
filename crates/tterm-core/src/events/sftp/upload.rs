use serde::Serialize;

/// A planned upload item, including its resolved destination.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadItemStartEvent {
    pub transfer_id: String,
    pub batch_id: Option<String>,
    pub file_name: String,
    pub file_size: u64,
    pub local_path: String,
    pub remote_path: String,
}

/// The original inputs and conflict policy of an upload batch.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadBatchStartEvent {
    pub batch_id: String,
    pub display_name: String,
    pub local_path: String,
    /// Every path the batch was started with, so a retry can re-run exactly
    /// this batch instead of whatever batch happens to be the latest one.
    pub local_paths: Vec<String>,
    pub remote_base_path: String,
    /// Echoed back so a retry re-runs the batch under the same policy.
    pub conflict_policy: crate::sftp::conflict::ConflictPolicy,
}

/// The final counters and cancellation state of an upload batch.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadBatchCompleteEvent {
    pub batch_id: String,
    pub cancelled: bool,
    pub error: Option<String>,
    pub failed: usize,
    pub succeeded: usize,
}

/// A sampled upload progress snapshot from the transfer engine.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadItemProgressEvent {
    pub transfer_id: String,
    pub local_path: String,
    pub transferred: u64,
    pub total: u64,
    pub progress: u32,
    pub resumed_from: u64,
    pub parallelism: u32,
    pub completed_chunks: u64,
    pub chunk_count: u64,
}

/// The outcome of one upload, including skipped destinations.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadItemCompleteEvent {
    pub transfer_id: String,
    pub error: Option<String>,
    pub local_path: String,
    pub remote_path: String,
    pub cancelled: bool,
    pub success: bool,
    /// The destination already held the source bytes, so no bytes moved in
    /// this run. Reported so the UI does not derive a speed from them.
    pub skipped: bool,
}
