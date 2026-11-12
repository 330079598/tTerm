//! The exit report of a cold terminal tab as the WebView receives it.

use crate::session_sink::TauriSessionSink;
use std::sync::{Arc, Mutex};
use tauri::Listener;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tterm_core::session::sink::SessionSink;

#[test]
fn legacy_exit_preserves_null_empty_and_error_reasons() {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    app.listen_any("pty-exit-tab-6", move |event| {
        captured.lock().unwrap().push(event.payload().to_string());
    });
    let sink = TauriSessionSink::new(
        "tab-6".into(),
        app.handle().clone(),
        crate::session_sink::output::new(tauri::ipc::Channel::new(|_| Ok(()))),
    );
    for reason in [None, Some(""), Some("SSH channel closed")] {
        sink.exited(reason.map(str::to_string)).unwrap();
    }
    assert_eq!(
        *received.lock().unwrap(),
        ["null", r#""""#, r#""SSH channel closed""#]
    );
}
