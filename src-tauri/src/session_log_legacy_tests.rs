use super::*;
use serde_json::json;
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
fn legacy_log_contract() {
    check(
        "log_status",
        serde_json::to_value(TerminalLogStatus {
            enabled: true,
            directory: "/test/logs".into(),
            active_sessions: 1,
            recording_tab_ids: vec!["tab-测试".into()],
            total_size_bytes: 42,
            last_error: None,
        })
        .unwrap(),
    );
    check("log_global", json!({"tabId":null,"message":"disk"}));
    check("log_error", json!({"tabId":"tab-测试","message":"disk"}));
    check(
        "log_start",
        json!({"tabId":"tab-测试","message":"disk","phase":"start"}),
    );
}
