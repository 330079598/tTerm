use super::*;
use serde_json::Value;
fn check(key: &str, actual: Value) {
    let fixtures: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../src-tauri/src/event_sink/fixtures/p1_03.json"
    )))
    .unwrap();
    assert_eq!(actual, fixtures[key], "{key}");
}
#[test]
fn legacy_cwd_contract() {
    check(
        "cwd",
        serde_json::to_value(CwdPayload {
            session_nonce: 7,
            cwd: "/test/目录".into(),
        })
        .unwrap(),
    );
}

#[test]
fn cwd_changes_are_deduplicated_even_after_delivery_failure() {
    use std::sync::Mutex;
    struct Sink(Mutex<Vec<AppEvent>>);
    impl EventSink for Sink {
        fn emit(&self, event: AppEvent) -> Result<(), String> {
            self.0.lock().unwrap().push(event);
            Err("closed".into())
        }
    }
    let sink = Sink(Mutex::new(Vec::new()));
    let mut last = None;
    emit_changed_cwd(&sink, "tab", 7, &mut last, "/first".into());
    emit_changed_cwd(&sink, "tab", 7, &mut last, "/first".into());
    emit_changed_cwd(&sink, "tab", 7, &mut last, "/second".into());
    assert_eq!(sink.0.lock().unwrap().len(), 2);
    assert_eq!(last.as_deref(), Some("/second"));
}
