//! Bounded OSC 52 store recognition on the shared control-string framing.
//!
//! This scanner never answers a query and never touches Alacritty's write
//! path. It returns completed stores with their end offsets so the session can
//! advance the main parser through the same bytes before applying them.

use crate::control_strings::{CompletedControlString, ControlStringFramer};
use crate::osc52::parse_osc52_copy;

/// Recognizes complete OSC 52 stores when clipboard writes are enabled.
#[derive(Debug, Default)]
pub struct Osc52Scanner {
    framer: ControlStringFramer,
    enabled: bool,
}

impl Osc52Scanner {
    /// Creates a scanner. A disabled scanner does not decode payloads.
    pub fn new(enabled: bool) -> Self {
        Self {
            framer: ControlStringFramer::with_osc52_payload(enabled),
            enabled,
        }
    }

    /// Scans one chunk and returns each complete store with its end offset.
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<(usize, String)> {
        if !self.enabled {
            return Vec::new();
        }
        let mut stores = Vec::new();
        self.framer.scan(bytes, |event| {
            Self::record(event, &mut stores);
        });
        stores
    }

    fn record(event: CompletedControlString<'_>, stores: &mut Vec<(usize, String)>) {
        if event.code != Some(52) || !event.payload_valid {
            return;
        }
        if let Ok(data) = std::str::from_utf8(event.payload)
            && let Some(text) = parse_osc52_copy(data)
        {
            stores.push((event.end, text));
        }
    }
}

#[cfg(test)]
mod tests;
