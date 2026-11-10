use serde::Serialize;

#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DeleteMethod {
    Sftp,
    Command,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchStartEvent {
    pub batch_id: String,
    pub entries: Vec<String>,
    pub method: DeleteMethod,
    pub total_directories: usize,
    pub total_entries: usize,
    pub total_files: usize,
    pub total_truncated: bool,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProgressEvent {
    pub batch_id: String,
    pub current_path: String,
    pub deleted_directories: usize,
    pub deleted_files: usize,
    pub failed: usize,
    pub method: DeleteMethod,
    pub total_directories: usize,
    pub total_entries: usize,
    pub total_files: usize,
    pub total_truncated: bool,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchCompleteEvent {
    pub batch_id: String,
    pub cancelled: bool,
    pub deleted_directories: usize,
    pub deleted_files: usize,
    pub error: Option<String>,
    pub failed: usize,
    pub method: DeleteMethod,
    pub total_directories: usize,
    pub total_entries: usize,
    pub total_files: usize,
    pub total_truncated: bool,
}
