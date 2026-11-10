use tterm_core::events::zmodem::{
    TransferCompletePayload, TransferProgressPayload, TransferStartPayload,
};

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
fn legacy_transfer_contract() {
    for (key, direction, file_size, size_unknown) in [
        ("start", "upload", 42, false),
        ("start_unknown", "download", 0, true),
    ] {
        check(
            key,
            serde_json::to_value(TransferStartPayload {
                transfer_id: "transfer-1".into(),
                direction,
                file_name: "测试.bin".into(),
                file_size,
                local_path: "/test/file".into(),
                size_unknown,
            })
            .unwrap(),
        );
    }
    check(
        "progress",
        serde_json::to_value(TransferProgressPayload {
            transfer_id: "transfer-1".into(),
            transferred: 42,
            speed: 21,
        })
        .unwrap(),
    );
    for (key, success, cancelled, error) in [
        ("complete", true, false, None),
        ("cancelled", false, true, Some("cancel".into())),
    ] {
        check(
            key,
            serde_json::to_value(TransferCompletePayload {
                transfer_id: "transfer-1".into(),
                success,
                cancelled,
                error,
            })
            .unwrap(),
        );
    }
}
