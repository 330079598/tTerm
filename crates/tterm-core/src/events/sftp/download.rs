use serde::Serialize;

/// A single-file or aggregate download progress snapshot.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    /// Identifies either the individual file or the aggregate batch progress.
    pub transfer_id: String,
    pub local_path: String,
    pub remote_path: String,
    /// Accounted bytes, including bytes already available locally.
    pub transferred: u64,
    /// The file size or aggregate batch size in bytes.
    pub total: u64,
    /// Integer completion percentage in the range 0 through 100.
    pub progress: u32,
    /// Bytes already present locally, excluded from this run's speed.
    /// Aggregate snapshots also include earlier skipped and resumed bytes.
    pub resumed_from: u64,
    /// Transfer lanes selected for the current file, or zero for skipped items
    /// and final summaries.
    pub parallelism: u32,
    /// Completed chunks for the current file, or zero for skipped items and
    /// final summaries.
    pub completed_chunks: u64,
    /// Planned chunks for the current file, or zero for skipped items and final
    /// summaries.
    pub chunk_count: u64,
}

/// A download item with its resolved local destination.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItemStartEvent {
    pub transfer_id: String,
    pub batch_id: String,
    pub file_name: String,
    /// The remote source size in bytes.
    pub file_size: u64,
    /// The destination after applying the conflict policy.
    pub local_path: String,
    pub remote_path: String,
}

/// The outcome of one download item.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItemCompleteEvent {
    pub transfer_id: String,
    pub error: Option<String>,
    pub local_path: String,
    pub remote_path: String,
    pub cancelled: bool,
    pub success: bool,
    /// The local file already held the remote bytes, so no bytes moved in
    /// this run. Reported so the UI does not derive a speed from them.
    pub skipped: bool,
}

/// Download batch totals, including bytes already present locally.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadBatchCompleteEvent {
    pub batch_id: String,
    pub cancelled: bool,
    pub error: Option<String>,
    /// Accounted bytes, including the skipped and resumed amounts below.
    pub transferred: u64,
    /// The aggregate planned size in bytes.
    pub total: u64,
    /// Bytes of files the run skipped because the local copy was already
    /// current. They are included in `transferred` so the bar reaches 100%,
    /// but no bytes moved for them — reported separately so the UI does not
    /// derive a speed from them.
    pub skipped: u64,
    /// Bytes of files this run resumed from a previous partial attempt: they
    /// are part of `transferred` as well, but only the remainder was fetched.
    pub resumed: u64,
}
