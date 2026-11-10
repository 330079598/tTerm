#[test]
fn legacy_static_contracts() {
    let source = include_str!("../background/quit.rs");
    assert!(source.contains("\"app-will-quit\""));
    assert_eq!(serde_json::to_value(()).unwrap(), serde_json::Value::Null);
}
