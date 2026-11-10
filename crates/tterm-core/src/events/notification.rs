use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivatedPayload {
    pub tab_id: Option<String>,
}

use super::{EventName, EventTarget};

/// P1-03: typed notification notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationEvent {
    Activated(ActivatedPayload),
}
impl NotificationEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Activated(_) => EventName::Static("notification-activated"),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Default
    }
}
