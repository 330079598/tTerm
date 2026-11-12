//! Wire contracts captured from Tauri's event transport before P1-03 and
//! P1-04 introduced the typed events behind them.
//!
//! They live beside [`crate::event_sink::TauriEventSink`] because they assert
//! the event name, target and JSON payload the WebView receives, which only
//! the Tauri transport decides.

mod sftp_delete;
mod sftp_download;
mod sftp_external;
mod sftp_search;
mod sftp_upload;
mod terminal_legacy_output;
