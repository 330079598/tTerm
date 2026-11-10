use super::{EventName, EventTarget};

/// P1-03: typed launch_dir notifications with the existing delivery contract.
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchDirEvent {
    Queued,
}
impl LaunchDirEvent {
    /// Returns the legacy transport name.
    pub fn name(&self) -> EventName {
        match self {
            Self::Queued => EventName::Static("launch-dirs"),
        }
    }
    /// Returns the event's delivery scope.
    pub fn target(&self) -> EventTarget {
        EventTarget::Default
    }
}
