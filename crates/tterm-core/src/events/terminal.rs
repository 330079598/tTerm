use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CwdPayload {
    pub session_nonce: u32,
    pub cwd: String,
}
