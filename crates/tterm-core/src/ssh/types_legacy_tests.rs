use super::*;
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
fn legacy_progress_contract() {
    let p = SshConnectionProgressPayload::new("ready", "ok");
    check("progress_min", serde_json::to_value(p).unwrap());
    let mut p = SshConnectionProgressPayload::new("retrying", "retry")
        .hop(1, 2)
        .host("localhost", 22303)
        .username("alice")
        .network_latency(Some(3))
        .retry_attempt(2, 1.5)
        .retry_max(4)
        .reason("drop")
        .shell_integration(Some(crate::events::ssh::ShellIntegrationStatus::Active {
            hostname: Some("test".into()),
            shell: Some("bash".into()),
        }));
    p.auth = Some(AuthUsedPayload {
        method: "key",
        private_key_path: Some("/test/key".into()),
        reusable: true,
    });
    check("progress_full", serde_json::to_value(p).unwrap());
}
