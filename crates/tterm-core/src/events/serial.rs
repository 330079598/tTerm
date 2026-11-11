use crate::events::zmodem::TransferEvent;

use super::{EventName, EventTarget};

/// P1-03: typed serial notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum SerialEvent {
    /// Reports a tab's serial file transfer through the shared transfer contract.
    Transfer {
        tab_id: String,
        event: TransferEvent,
    },
}
impl SerialEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Transfer { tab_id, event } => event.name(tab_id),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Any
    }
}
