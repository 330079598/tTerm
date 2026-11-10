#[test]
fn legacy_static_contracts() {
    let source = include_str!("../../../crates/tterm-core/src/events/background.rs");
    assert!(source.contains("\"app-will-quit\""));
    assert_eq!(serde_json::to_value(()).unwrap(), serde_json::Value::Null);
}
