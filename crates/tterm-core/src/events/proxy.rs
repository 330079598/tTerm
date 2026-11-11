use serde::Serialize;

/// A question the user is asked before a proxy command first runs here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCommandPrompt {
    /// The SHA-256 of the command, so that connections waiting on the same
    /// command share one question.
    pub request_id: String,
    pub proxy_name: String,
    pub command: String,
}
