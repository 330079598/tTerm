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
fn legacy_blockers_contract() {
    check(
        "blockers",
        serde_json::to_value(QuitBlockers {
            tunnels: 2,
            unuploaded_files: vec!["test.conf".into()],
        })
        .unwrap(),
    );
}

#[test]
fn held_confirmation_precedes_save_and_cancellation_rearms_it() {
    use std::sync::Mutex;
    struct Sink(Mutex<Vec<AppEvent>>);
    impl EventSink for Sink {
        fn emit(&self, event: AppEvent) -> Result<(), String> {
            self.0.lock().unwrap().push(event);
            Err("closed".into())
        }
    }
    let state = ExitGuardState::default();
    let sink = Sink(Mutex::new(Vec::new()));
    let blockers = QuitBlockers {
        tunnels: 1,
        ..QuitBlockers::default()
    };
    assert!(emit_confirmation(&state, blockers.clone(), &sink));
    assert!(!emit_confirmation(&state, blockers.clone(), &sink));
    assert_eq!(
        *sink.0.lock().unwrap(),
        vec![AppEvent::Background(BackgroundEvent::QuitRequested(
            blockers.clone()
        ))]
    );
    state.prompt_open.store(false, Ordering::SeqCst);
    assert!(emit_confirmation(&state, blockers.clone(), &sink));
    state.confirmed.store(true, Ordering::SeqCst);
    assert!(!emit_confirmation(&state, blockers, &sink));
    assert_eq!(sink.0.lock().unwrap().len(), 2);
}
