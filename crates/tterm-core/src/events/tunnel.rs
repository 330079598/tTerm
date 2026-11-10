use serde::Serialize;

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelState {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error,
    /// An automatic start was skipped because a secret is missing.
    #[serde(rename = "needsCredentials")]
    NeedsCredentials,
}

impl TunnelState {
    /// Running, or on its way to running.
    pub fn is_active(self) -> bool {
        !matches!(self, Self::Stopped | Self::Error | Self::NeedsCredentials)
    }
}

/// Live status snapshot sent to the UI on the `tunnel-status` event.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub id: String,
    pub state: TunnelState,
    pub message: Option<String>,
    /// Port actually bound (differs from the rule for server-assigned ports).
    pub bound_port: Option<u16>,
    pub active_connections: u64,
    pub total_connections: u64,
    /// Connections that could not reach their destination.
    pub failed_connections: u64,
    /// Bytes sent toward the destination / received back from it.
    pub bytes_up: u64,
    pub bytes_down: u64,
    /// Unix ms when the current session came up.
    pub connected_at: Option<u64>,
    pub retry_attempt: u32,
    /// Why the most recent forwarded connection failed, kept while the tunnel
    /// itself stays up so a refused destination is not invisible.
    pub last_failure: Option<TunnelFailure>,
    /// Totals across every run, including the current one.
    pub lifetime: TunnelTraffic,
}

/// Traffic a tunnel has carried across runs, saved in the database.
#[derive(Debug, Serialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunnelTraffic {
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub connections: u64,
    /// Unix ms when counting began: the first run counted or the last reset.
    pub since: Option<u64>,
}

impl TunnelTraffic {
    /// These totals with `delta` (up, down, connections) added.
    pub fn plus(self, (up, down, connections): (u64, u64, u64)) -> Self {
        Self {
            bytes_up: self.bytes_up.saturating_add(up),
            bytes_down: self.bytes_down.saturating_add(down),
            connections: self.connections.saturating_add(connections),
            since: self.since,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunnelFailure {
    pub message: String,
    /// Unix ms.
    pub at: u64,
}

impl TunnelStatus {
    pub fn stopped(id: &str, lifetime: TunnelTraffic) -> Self {
        Self {
            id: id.to_string(),
            state: TunnelState::Stopped,
            message: None,
            bound_port: None,
            active_connections: 0,
            total_connections: 0,
            failed_connections: 0,
            bytes_up: 0,
            bytes_down: 0,
            connected_at: None,
            retry_attempt: 0,
            last_failure: None,
            lifetime,
        }
    }
}

/// Tunnel events emitted to the application shell.
#[derive(Debug, Clone, PartialEq)]
pub enum TunnelEvent {
    /// A live status snapshot changed.
    Status(TunnelStatus),
}

impl TunnelEvent {
    /// Tunnel status historically used `AppHandle::emit`, which targets the
    /// application default and is intentionally different from `emit_to(any)`.
    pub fn target(&self) -> super::EventTarget {
        match self {
            Self::Status(_) => super::EventTarget::Default,
        }
    }

    pub fn name(&self) -> super::EventName {
        match self {
            Self::Status(_) => super::EventName::Static("tunnel-status"),
        }
    }
}
