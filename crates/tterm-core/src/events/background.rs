use serde::Serialize;

/// What a quit would end before the user confirms it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuitBlockers {
    /// Port-forwarding tunnels running.
    pub tunnels: usize,
    /// Files opened from SFTP with changes not uploaded; their copies stay
    /// on disk, but nothing uploads them any more.
    pub unuploaded_files: Vec<String>,
}

impl QuitBlockers {
    /// Returns whether quitting would leave no running tunnels or unuploaded files.
    pub fn is_empty(&self) -> bool {
        self.tunnels == 0 && self.unuploaded_files.is_empty()
    }
}

use super::{EventName, EventTarget};

/// P1-03: typed background notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundEvent {
    /// Asks the user to confirm quitting with the listed blockers.
    QuitRequested(QuitBlockers),
    /// Asks the user how to handle a window close request.
    CloseRequested,
    /// Gives the frontend a chance to save before the application exits.
    WillQuit,
    /// Requests the tunnels panel from the tray menu.
    OpenTunnels,
    /// Requests an update check from the tray menu.
    CheckUpdates,
}
impl BackgroundEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::QuitRequested(_) => EventName::Static("quit-requested"),
            Self::CloseRequested => EventName::Static("close-requested"),
            Self::WillQuit => EventName::Static("app-will-quit"),
            Self::OpenTunnels => EventName::Static("tray-open-tunnels"),
            Self::CheckUpdates => EventName::Static("tray-check-updates"),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Default
    }
}
