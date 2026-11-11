//! P1-06: Tauri's tab-scoped raw output and lifecycle endpoint.
pub(crate) mod output;
use crate::event_sink::TauriEventSink;
use crate::terminal::output::TerminalOutput;
use tauri::{AppHandle, Runtime};
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::session::sink::SessionSink;

/// Keeps output flow shared across attempts and page reattachments.
pub(crate) struct TauriSessionSink<R: Runtime = tauri::Wry> {
    tab_id: String,
    events: TauriEventSink<R>,
    output: TerminalOutput,
}

impl<R: Runtime> TauriSessionSink<R> {
    pub(crate) fn new(tab_id: String, app: AppHandle<R>, output: TerminalOutput) -> Self {
        Self {
            tab_id,
            events: TauriEventSink::new(app),
            output,
        }
    }
}

impl<R: Runtime> EventSink for TauriSessionSink<R> {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.events.emit(event)
    }
}

impl<R: Runtime> SessionSink for TauriSessionSink<R> {
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.output.send(bytes)
    }

    fn state_changed(&self, payload: SshConnectionProgressPayload) -> Result<(), String> {
        self.emit(AppEvent::Ssh(SshEvent::ConnectionProgress {
            tab_id: self.tab_id.clone(),
            payload,
        }))
    }

    fn exited(&self, reason: Option<String>) -> Result<(), String> {
        self.emit(AppEvent::Terminal(TerminalEvent::Exit {
            tab_id: self.tab_id.clone(),
            reason,
        }))
    }

    fn ack_output(&self, bytes: usize) {
        self.output.flow().ack(bytes);
    }
    fn close_output(&self) {
        self.output.flow().close();
    }
    fn note_input(&self) {
        self.output.note_input();
    }
    fn answers_input(&self) -> bool {
        self.output.answers_input()
    }
}

#[cfg(test)]
mod tests;
