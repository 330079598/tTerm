//! A bounded-use recorder for tests, never a production flow-control algorithm.

use crate::events::{AppEvent, EventSink};
use crate::session::sink::SessionSink;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// An observed endpoint call, in the order the recorder received it.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionRecord {
    Bytes(Vec<u8>),
    Event(AppEvent),
    Exit(Option<String>),
    Ack(usize),
    Close,
    Input,
}

/// Records endpoint calls without automatically acknowledging output.
///
/// Flow-control tests must compose this with production OutputFlow, rather
/// than reproducing the watermark algorithm in a fake implementation.
#[derive(Default)]
pub struct RecordingSessionSink {
    state: Mutex<RecordingState>,
}

#[derive(Default)]
struct RecordingState {
    records: Vec<SessionRecord>,
    last_input: Option<Instant>,
}

impl RecordingSessionSink {
    /// Returns a snapshot of all recorded calls.
    pub fn records(&self) -> Vec<SessionRecord> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .records
            .clone()
    }

    fn record(&self, record: SessionRecord) {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .records
            .push(record);
    }
}

impl EventSink for RecordingSessionSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.record(SessionRecord::Event(event));
        Ok(())
    }
}

impl SessionSink for RecordingSessionSink {
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.record(SessionRecord::Bytes(bytes));
        Ok(())
    }

    fn exited(&self, reason: Option<String>) -> Result<(), String> {
        self.record(SessionRecord::Exit(reason));
        Ok(())
    }

    fn ack_output(&self, bytes: usize) {
        self.record(SessionRecord::Ack(bytes));
    }

    fn close_output(&self) {
        self.record(SessionRecord::Close);
    }

    fn note_input(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.last_input = Some(Instant::now());
        state.records.push(SessionRecord::Input);
    }

    fn answers_input(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last_input
            .is_some_and(|at| at.elapsed() < Duration::from_millis(150))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_preserves_bytes_reasons_and_explicit_ack_order() {
        let sink = RecordingSessionSink::default();
        let endpoint: &dyn SessionSink = &sink;
        assert!(!endpoint.answers_input());
        endpoint.send_bytes(vec![0, 255, 13, 10]).unwrap();
        endpoint.ack_output(4);
        endpoint.note_input();
        assert!(endpoint.answers_input());
        endpoint.close_output();
        endpoint.exited(None).unwrap();
        endpoint.exited(Some("".into())).unwrap();
        assert_eq!(
            sink.records(),
            vec![
                SessionRecord::Bytes(vec![0, 255, 13, 10]),
                SessionRecord::Ack(4),
                SessionRecord::Input,
                SessionRecord::Close,
                SessionRecord::Exit(None),
                SessionRecord::Exit(Some("".into())),
            ]
        );
    }
}
