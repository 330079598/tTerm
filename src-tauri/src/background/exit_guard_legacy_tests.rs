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
