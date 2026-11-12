use super::{HighlightRule, HighlightSet, KeywordHighlightConfig};

#[test]
fn config_roundtrip_matches_backend_shape() {
    let backend = KeywordHighlightConfig {
        default_set: "net".into(),
        sets: Some(vec![HighlightSet {
            id: "net".into(),
            name: "Network".into(),
            rules: vec![HighlightRule {
                id: "r1".into(),
                pattern: "down".into(),
                foreground: Some("#ff0000".into()),
                ..Default::default()
            }],
        }]),
        ..Default::default()
    };
    for backend in [backend, KeywordHighlightConfig::default()] {
        let backend_json = serde_json::to_string(&backend).unwrap();
        let core: crate::config::keyword_highlight::KeywordHighlightConfig =
            serde_json::from_str(&backend_json).unwrap();
        let core_json = serde_json::to_string(&core).unwrap();
        assert_eq!(core_json.as_bytes(), backend_json.as_bytes());
        let reread: KeywordHighlightConfig = serde_json::from_str(&core_json).unwrap();
        assert_eq!(reread, backend);
    }
}
