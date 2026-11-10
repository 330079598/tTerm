use serde::Serialize;

/// A single-file or aggregate download progress snapshot.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub transfer_id: String,
    pub local_path: String,
    pub remote_path: String,
    pub transferred: u64,
    pub total: u64,
    pub progress: u32,
    pub resumed_from: u64,
    pub parallelism: u32,
    pub completed_chunks: u64,
    pub chunk_count: u64,
}

/// A download item with its resolved local destination.
#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItemStartEvent {
    pub transfer_id: String,
    pub batch_id: String,
    pub file_name: String,
    pub file_size: u64,
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
    pub transferred: u64,
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
