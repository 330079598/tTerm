use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelKind {
    /// `ssh -L`: listen locally, connect to the destination from the server.
    Local,
    /// `ssh -R`: listen on the server, connect to the destination from here.
    Remote,
    /// `ssh -D`: local SOCKS5 proxy whose connections leave from the server.
    Dynamic,
}

/// A saved port-forwarding rule bound to a saved SSH profile.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelRule {
    pub id: String,
    pub name: String,
    pub profile_id: String,
    pub kind: TunnelKind,
    /// Interface to listen on: local for `Local`/`Dynamic`, remote for `Remote`.
    pub bind_host: String,
    pub bind_port: u16,
    /// Forward destination; unused for `Dynamic`.
    #[serde(default)]
    pub dest_host: String,
    #[serde(default)]
    pub dest_port: u16,
}

impl TunnelRule {
    pub fn validate(&mut self) -> Result<(), String> {
        self.name = self.name.trim().to_string();
        self.bind_host = self.bind_host.trim().to_string();
        self.dest_host = self.dest_host.trim().to_string();

        if self.id.trim().is_empty() {
            return Err("Tunnel id is required".to_string());
        }
        if self.name.is_empty() {
            return Err("Tunnel name is required".to_string());
        }
        if self.profile_id.trim().is_empty() {
            return Err("Choose a host for the tunnel".to_string());
        }
        if self.bind_host.is_empty() {
            return Err("Bind address is required".to_string());
        }
        if self.bind_port == 0 {
            return Err("Bind port must be between 1 and 65535".to_string());
        }
        if self.kind == TunnelKind::Dynamic {
            self.dest_host.clear();
            self.dest_port = 0;
        } else {
            if self.dest_host.is_empty() {
                return Err("Destination host is required".to_string());
            }
            if self.dest_port == 0 {
                return Err("Destination port must be between 1 and 65535".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TunnelState {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error,
}

/// Live status snapshot sent to the UI on the `tunnel-status` event.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub id: String,
    pub state: TunnelState,
    pub message: Option<String>,
    /// Port actually bound (differs from the rule for server-assigned ports).
    pub bound_port: Option<u16>,
    pub active_connections: u64,
    pub total_connections: u64,
    /// Bytes sent toward the destination / received back from it.
    pub bytes_up: u64,
    pub bytes_down: u64,
    /// Unix ms when the current session came up.
    pub connected_at: Option<u64>,
    pub retry_attempt: u32,
}

impl TunnelStatus {
    pub fn stopped(id: &str) -> Self {
        Self {
            id: id.to_string(),
            state: TunnelState::Stopped,
            message: None,
            bound_port: None,
            active_connections: 0,
            total_connections: 0,
            bytes_up: 0,
            bytes_down: 0,
            connected_at: None,
            retry_attempt: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct TunnelStats {
    pub active_connections: AtomicU64,
    pub total_connections: AtomicU64,
    pub bytes_up: AtomicU64,
    pub bytes_down: AtomicU64,
}

impl TunnelStats {
    pub fn reset(&self) {
        self.active_connections.store(0, Ordering::Relaxed);
        self.total_connections.store(0, Ordering::Relaxed);
        self.bytes_up.store(0, Ordering::Relaxed);
        self.bytes_down.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: TunnelKind) -> TunnelRule {
        TunnelRule {
            id: "t1".into(),
            name: " db ".into(),
            profile_id: "p1".into(),
            kind,
            bind_host: " 127.0.0.1 ".into(),
            bind_port: 5432,
            dest_host: " db.internal ".into(),
            dest_port: 5432,
        }
    }

    #[test]
    fn validate_trims_and_accepts_a_local_rule() {
        let mut r = rule(TunnelKind::Local);
        r.validate().expect("valid");
        assert_eq!(r.name, "db");
        assert_eq!(r.bind_host, "127.0.0.1");
        assert_eq!(r.dest_host, "db.internal");
    }

    #[test]
    fn validate_requires_a_destination_except_for_dynamic() {
        let mut local = rule(TunnelKind::Local);
        local.dest_host.clear();
        assert!(local.validate().is_err());

        let mut remote = rule(TunnelKind::Remote);
        remote.dest_port = 0;
        assert!(remote.validate().is_err());

        let mut dynamic = rule(TunnelKind::Dynamic);
        dynamic.validate().expect("dynamic needs no destination");
        assert!(dynamic.dest_host.is_empty());
        assert_eq!(dynamic.dest_port, 0);
    }

    #[test]
    fn validate_rejects_missing_fields() {
        let mut r = rule(TunnelKind::Local);
        r.bind_port = 0;
        assert!(r.validate().is_err());

        let mut r = rule(TunnelKind::Local);
        r.profile_id.clear();
        assert!(r.validate().is_err());
    }
}
