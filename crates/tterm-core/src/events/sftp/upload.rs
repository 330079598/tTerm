use serde::Serialize;

/// A planned upload item, including its resolved destination.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadItemStartEvent {
    pub transfer_id: String,
    /// The containing batch, if this item belongs to a batch upload.
    pub batch_id: Option<String>,
    pub file_name: String,
    /// The source file size in bytes.
    pub file_size: u64,
    pub local_path: String,
    /// The destination after applying the conflict policy.
    pub remote_path: String,
}

/// The original inputs and conflict policy of an upload batch.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UploadBatchStartEvent {
    pub batch_id: String,
    /// The batch label shown in the transfer list.
    pub display_name: String,
    /// The primary source path retained alongside the full retry inputs.
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
    /// Accounted bytes, including bytes retained from an earlier attempt.
    pub transferred: u64,
    /// The source file size in bytes.
    pub total: u64,
    /// Integer completion percentage in the range 0 through 100.
    pub progress: u32,
    /// Bytes retained from an earlier attempt, excluded from this run's speed.
    pub resumed_from: u64,
    /// The number of transfer lanes selected by the engine.
    pub parallelism: u32,
    /// Completed chunks, including those recovered from a previous attempt.
    pub completed_chunks: u64,
    /// The total number of chunks planned for this file.
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
