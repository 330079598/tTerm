use super::*;
use crate::events::ssh::LinuxDistributionInfo;
use serde_json::Value;
fn check(key: &str, actual: Value) {
    let fixtures: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src-tauri/src/event_sink/fixtures/p1_03.json"
    )))
    .unwrap();
    assert_eq!(actual, fixtures[key], "{key}");
}
#[test]
fn legacy_os_contract() {
    check(
        "os_none",
        serde_json::to_value(RemoteOsPayload {
            tab_id: "tab-测试".into(),
            distribution: None,
        })
        .unwrap(),
    );
    check(
        "os_full",
        serde_json::to_value(RemoteOsPayload {
            tab_id: "tab-测试".into(),
            distribution: Some(LinuxDistributionInfo {
                id: "ubuntu".into(),
                id_like: vec!["debian".into()],
                name: "Ubuntu".into(),
                pretty_name: "Ubuntu Test".into(),
                version_id: None,
            }),
        })
        .unwrap(),
    );
}
