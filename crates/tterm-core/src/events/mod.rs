//! Typed business events and delivery interfaces shared by the Tauri and GPUI clients.

pub mod background;
pub mod notification;
pub mod session_log;
pub mod ssh;
pub mod terminal;
pub mod tunnel;
pub mod zmodem;

use background::BackgroundEvent;
use ssh::SshEvent;
use tunnel::TunnelEvent;

/// A business event emitted by a core service.
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// Tunnel state and traffic changed.
    Tunnel(TunnelEvent),
    /// P1-03: background event delivery.
    Background(BackgroundEvent),
    /// P1-03: ssh event delivery.
    Ssh(SshEvent),
}

/// The logical delivery scope of an event.
///
/// `Default` preserves `AppHandle::emit`, while `Any` preserves
/// `AppHandle::emit_to(EventTarget::any(), ...)` for events that are routed to
/// every webview. The Tauri adapter owns the concrete target type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTarget {
    Default,
    Any,
}

/// A stable or tab-scoped wire event name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventName {
    Static(&'static str),
    TabScoped {
        prefix: &'static str,
        tab_id: String,
    },
}

impl EventName {
    /// Builds the client event name, including a dynamic tab suffix when one
    /// is part of the event contract.
    pub fn as_string(&self) -> String {
        match self {
            Self::Static(name) => (*name).to_string(),
            Self::TabScoped { prefix, tab_id } => format!("{prefix}-{tab_id}"),
        }
    }
}

impl AppEvent {
    /// Returns the delivery scope required by this event.
    pub fn target(&self) -> EventTarget {
        match self {
            Self::Tunnel(event) => event.target(),
            Self::Background(event) => event.target(), // P1-03.
            Self::Ssh(event) => event.target(),        // P1-03.
        }
    }

    /// Returns this event's stable or dynamic wire name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Tunnel(event) => event.name(),
            Self::Background(event) => event.name(), // P1-03.
            Self::Ssh(event) => event.name(),        // P1-03.
        }
    }
}

/// Receives typed business events from core services.
pub trait EventSink: Send + Sync {
    /// Delivers one event and reports adapter failures to the caller.
    fn emit(&self, event: AppEvent) -> Result<(), String>;
}

#[cfg(any(test, feature = "test-support"))]
pub mod recording;
