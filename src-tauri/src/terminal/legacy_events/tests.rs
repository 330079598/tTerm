use super::*;
use std::sync::{Arc, Mutex};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::Listener;

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
        emit_output(app.handle(), "tab-with-hyphens", text.to_string());
    }
    assert_eq!(TARGET, tauri::EventTarget::Any);
    assert_eq!(
        *received.lock().unwrap(),
        cases.map(|(_, json)| json.to_string())
    );
}

#[test]
fn legacy_exit_preserves_null_empty_and_error_reasons() {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    app.listen_any("pty-exit-tab-6", move |event| {
        captured.lock().unwrap().push(event.payload().to_string());
    });
    for reason in [None, Some(""), Some("SSH channel closed")] {
        emit_exit(app.handle(), "tab-6", reason);
    }
    assert_eq!(TARGET, tauri::EventTarget::Any);
    assert_eq!(
        *received.lock().unwrap(),
        ["null", r#""""#, r#""SSH channel closed""#]
    );
}

#[test]
fn legacy_status_formatting_and_lossy_fallback_are_frozen() {
    assert_eq!(
        status_line("33", "Connecting"),
        "\r\n\x1b[33m[Connecting]\x1b[0m\r\n"
    );
    assert_eq!(
        jump_status_line("31", 2, "rejected"),
        "\r\n\x1b[31m[Jump #2: rejected]\x1b[0m\r\n"
    );
    assert_eq!(String::from_utf8_lossy(b"bad:\xff"), "bad:�");
}
