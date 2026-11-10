use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionProgressPayload {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hops: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_latency_ms: Option<u64>,
    /// 1-indexed reconnect attempt for the `retrying` phase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_attempt: Option<u32>,
    /// Delay chosen for the pending reconnect attempt, in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_delay_secs: Option<f64>,
    /// Configured reconnect attempt limit for the retry phases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_max_attempts: Option<u32>,
    /// Why the session dropped, for the retry phases. The frontend renders the
    /// localized retry status from the structured fields, not `message`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How shell integration went, for the `ready` phase of a connection
    /// that turned it on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_integration: Option<ShellIntegrationStatus>,
    /// What automatic authentication found to work, for the
    /// `target_authenticated` phase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthUsedPayload>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUsedPayload {
    /// The `auth_method` a profile would store for it.
    pub method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
    /// Connecting the same way again needs nothing typed.
    pub reusable: bool,
}

impl SshConnectionProgressPayload {
    pub fn new(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            phase: phase.into(),
            message: message.into(),
            hop_index: None,
            total_hops: None,
            host: None,
            port: None,
            username: None,
            network_latency_ms: None,
            retry_attempt: None,
            retry_delay_secs: None,
            retry_max_attempts: None,
            reason: None,
            shell_integration: None,
            auth: None,
        }
    }

    pub fn host(mut self, host: impl Into<String>, port: u16) -> Self {
        self.host = Some(host.into());
        self.port = Some(port);
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn hop(mut self, hop_index: usize, total_hops: usize) -> Self {
        self.hop_index = Some(hop_index);
        self.total_hops = Some(total_hops);
        self
    }

    pub fn network_latency(mut self, network_latency_ms: Option<u64>) -> Self {
        self.network_latency_ms = network_latency_ms;
        self
    }

    pub fn shell_integration(mut self, status: Option<ShellIntegrationStatus>) -> Self {
        self.shell_integration = status;
        self
    }

    /// Describe the pending reconnect attempt (used by the `retrying` phase).
    pub fn retry_attempt(mut self, attempt: u32, delay_secs: f64) -> Self {
        self.retry_attempt = Some(attempt);
        self.retry_delay_secs = Some(delay_secs);
        self
    }

    /// Attach the configured reconnect attempt limit (retry phases).
    pub fn retry_max(mut self, max_attempts: u32) -> Self {
        self.retry_max_attempts = Some(max_attempts);
        self
    }

    /// Attach the disconnect reason (retry phases).
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// How shell integration went for a session, shown in its connection header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ShellIntegrationStatus {
    Active {
        /// The host's name (`uname -n`), which tells its directory reports
        /// apart from those of a host reached from it with `ssh`.
        #[serde(skip_serializing_if = "Option::is_none")]
        hostname: Option<String>,
        /// The login shell detected for this connection, never from a workspace.
        #[serde(skip_serializing_if = "Option::is_none")]
        shell: Option<String>,
    },
    /// The session got a plain shell.
    Unavailable { reason: UnavailableReason },
}

/// Why a session got a plain shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum UnavailableReason {
    /// The login shell is not bash, zsh or fish.
    Shell {
        shell: String,
    },
    /// Nothing ran the installer: no POSIX `sh` (a network device, a Windows
    /// server) or a forced command.
    NoReply,
    /// The scripts could not be written to the cache directory.
    WriteFailed,
    /// The cache directory's path holds a single quote.
    Path,
    /// The server refused to run the start script.
    Rejected,
    TimedOut,
    /// The SSH channel failed.
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDistributionInfo {
    pub id: String,
    pub id_like: Vec<String>,
    pub name: String,
    pub pretty_name: String,
    pub version_id: Option<String>,
}

/// Reports a host's distribution, or null when it cannot be identified.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOsPayload {
    pub tab_id: String,
    pub distribution: Option<LinuxDistributionInfo>,
}

use super::{EventName, EventTarget};

/// P1-03: typed ssh notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum SshEvent {
    ConnectionProgress {
        tab_id: String,
        payload: SshConnectionProgressPayload,
    },
    RemoteOs(RemoteOsPayload),
}
impl SshEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::ConnectionProgress { tab_id, .. } => EventName::TabScoped {
                prefix: "ssh-connection-progress",
                tab_id: tab_id.clone(),
            },
            Self::RemoteOs(_) => EventName::Static("ssh-remote-os"),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        match self {
            Self::ConnectionProgress { .. } => EventTarget::Any,
            Self::RemoteOs(_) => EventTarget::Default,
        }
    }
}
