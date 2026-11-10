use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferStartPayload {
    pub transfer_id: String,
    pub direction: &'static str,
    pub file_name: String,
    pub file_size: u64,
    pub local_path: String,
    /// The sender gave no size, as XMODEM never does: `file_size` is 0 and
    /// the list shows what has arrived without a total.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub size_unknown: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgressPayload {
    pub transfer_id: String,
    pub transferred: u64,
    pub speed: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferCompletePayload {
    pub transfer_id: String,
    pub success: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}
