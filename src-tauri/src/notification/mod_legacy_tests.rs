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
