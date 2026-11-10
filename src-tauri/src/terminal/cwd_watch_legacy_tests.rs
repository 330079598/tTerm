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
