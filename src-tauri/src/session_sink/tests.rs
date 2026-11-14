use crate::session_sink::*;
use std::sync::{Arc, Mutex};
use tauri::Listener;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::test::{mock_builder, mock_context, noop_assets};

#[test]
fn session_sink_preserves_raw_state_and_exit_contracts() {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let raw = captured.clone();
    let output = crate::session_sink::output::new(Channel::new(move |body| {
        let InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected raw output")
        };
        raw.lock().unwrap().push(bytes);
        Ok(())
    }));
    let sink = TauriSessionSink::new("test-tab".into(), app.handle().clone(), output);
    sink.send_bytes(vec![0, 255, 13, 10]).unwrap();
    assert_eq!(*captured.lock().unwrap(), vec![vec![0, 255, 13, 10]]);
    let received = Arc::new(Mutex::new(Vec::new()));
    for name in ["pty-exit-test-tab", "ssh-connection-progress-test-tab"] {
        let recorded = received.clone();
        app.listen_any(name, move |event| {
            recorded.lock().unwrap().push(event.payload().to_string())
        });
    }
    sink.state_changed(SshConnectionProgressPayload::new("ready", "connected"))
        .unwrap();
    sink.exited(None).unwrap();
    sink.exited(Some("".into())).unwrap();
    assert_eq!(
        *received.lock().unwrap(),
        [
            r#"{"phase":"ready","message":"connected"}"#,
            "null",
            r#""""#
        ]
    );
    assert!(!sink.answers_input());
    sink.note_input();
    assert!(sink.answers_input());
    sink.ack_output(usize::MAX);
    sink.close_output();
}

#[test]
fn typed_output_emit_reaches_listener_with_frozen_json() {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    app.listen_any("pty-output-tab-with-hyphens", move |event| {
        captured.lock().unwrap().push(event.payload().to_string());
    });
    let sink = TauriEventSink::new(app.handle().clone());
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
        sink.emit(AppEvent::Terminal(TerminalEvent::Output {
            tab_id: "tab-with-hyphens".into(),
            text: text.into(),
        }))
        .unwrap();
    }
    assert_eq!(
        *received.lock().unwrap(),
        cases.map(|(_, json)| json.to_string())
    );
}

#[test]
fn typed_terminal_wire_matches_frozen_legacy_literals() {
    use tterm_core::events::EventTarget;
    for (text, json) in [
        ("", r#""""#),
        ("中文\0\r\n\"\\", r#""中文\u0000\r\n\"\\""#),
        (
            "\r\n\x1b[31m[failed]\x1b[0m\r\n",
            r#""\r\n\u001b[31m[failed]\u001b[0m\r\n""#,
        ),
        ("bad:�", r#""bad:�""#),
    ] {
        let event = AppEvent::Terminal(TerminalEvent::Output {
            tab_id: "tab-with-hyphens".into(),
            text: text.into(),
        });
        let (name, target, value) = TauriEventSink::wire(&event).unwrap();
        assert_eq!(name.as_string(), "pty-output-tab-with-hyphens");
        assert_eq!(target, EventTarget::Any);
        assert_eq!(
            value,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
    for (reason, json) in [
        (None, "null"),
        (Some(""), r#""""#),
        (Some("SSH channel closed"), r#""SSH channel closed""#),
    ] {
        let event = AppEvent::Terminal(TerminalEvent::Exit {
            tab_id: "tab-6".into(),
            reason: reason.map(str::to_string),
        });
        let (name, target, value) = TauriEventSink::wire(&event).unwrap();
        assert_eq!(name.as_string(), "pty-exit-tab-6");
        assert_eq!(target, EventTarget::Any);
        assert_eq!(
            value,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
}
