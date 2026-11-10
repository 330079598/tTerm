use super::*;
use serde_json::Value;
fn check(key: &str, actual: Value) {
    let fixtures: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/event_sink/fixtures/p1_03.json"
    )))
    .unwrap();
    assert_eq!(actual, fixtures[key], "{key}");
}
#[test]
fn legacy_activation_contract() {
    for (key, tab_id) in [
        ("activated", Some("tab-测试".into())),
        ("activated_none", None),
    ] {
        check(
            key,
            serde_json::to_value(ActivatedPayload { tab_id }).unwrap(),
        );
    }
}

#[test]
fn activation_shows_window_before_emitting_even_when_delivery_fails() {
    use std::sync::{Arc, Mutex};
    struct Sink(Arc<Mutex<Vec<&'static str>>>);
    impl EventSink for Sink {
        fn emit(&self, event: AppEvent) -> Result<(), String> {
            assert!(matches!(event, AppEvent::Notification(_)));
            self.0.lock().unwrap().push("emit");
            Err("closed".into())
        }
    }
    let trace = Arc::new(Mutex::new(Vec::new()));
    let shown = trace.clone();
    activate_with_events(
        move || shown.lock().unwrap().push("show"),
        &Sink(trace.clone()),
        None,
    );
    assert_eq!(*trace.lock().unwrap(), vec!["show", "emit"]);
}
