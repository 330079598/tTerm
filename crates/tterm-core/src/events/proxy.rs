use serde::Serialize;

/// A question the user is asked before a proxy command first runs here.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCommandPrompt {
    /// The SHA-256 of the command, so that connections waiting on the same
    /// command share one question.
    pub request_id: String,
    pub proxy_name: String,
    pub command: String,
}

/// P1-05: approval questions for locally executed proxy commands.
#[derive(Debug, Clone, PartialEq)]
pub enum ProxyEvent {
    CommandPrompt(ProxyCommandPrompt),
    CommandPromptExpired(String),
}
impl ProxyEvent {
    /// Returns the existing frontend event name.
    pub fn name(&self) -> super::EventName {
        super::EventName::Static(match self {
            Self::CommandPrompt(_) => "proxy-command-prompt",
            Self::CommandPromptExpired(_) => "proxy-command-prompt-expired",
        })
    }
    /// Proxy approval is visible across webviews.
    pub fn target(&self) -> super::EventTarget {
        super::EventTarget::Any
    }
}
