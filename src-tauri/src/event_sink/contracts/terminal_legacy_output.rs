//! The output report of a cold terminal tab as the WebView receives it.

use crate::event_sink::TauriEventSink;
use std::sync::{Arc, Mutex};
use tauri::Listener;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tterm_core::terminal::legacy_events::emit_output;

#[test]
fn legacy_output_name_target_and_json_are_frozen_before_migration() {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    app.listen_any("pty-output-tab-with-hyphens", move |event| {
        captured.lock().unwrap().push(event.payload().to_string());
    });
    let cases = [
        ("", r#""""#),
        ("中文\0\r\n\"\\", r#""中文\u0000\r\n\"\\""#),
        (
            "\r\n\x1b[31m[failed]\x1b[0m\r\n",
            r#""\r\n\u001b[31m[failed]\u001b[0m\r\n""#,
        ),
        ("bad:�", r#""bad:�""#),
    ];
    for (text, _) in cases {
        emit_output(
            &TauriEventSink::new(app.handle().clone()),
            "tab-with-hyphens",
            text.to_string(),
        );
    }
    assert_eq!(
        *received.lock().unwrap(),
        cases.map(|(_, json)| json.to_string())
    );
}
