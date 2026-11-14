use serde_json::{Value, json};

use super::{
    DEFAULT_TAB_STANDARD_WIDTH, INACTIVE_PANE_DIM_OPTIONS, default_tab_settings,
    normalize_tab_settings,
};

/// The expectations generated from the real TypeScript module by
/// `scripts/migration/P1-11c-oracle.mjs`.
fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

#[test]
fn knows_only_the_two_width_modes() {
    assert_eq!(
        normalize_tab_settings(&json!({"tab_width_mode": "standard"})).tab_width_mode,
        "standard"
    );
    assert_eq!(
        normalize_tab_settings(&json!({"tab_width_mode": "wide"})).tab_width_mode,
        "adaptive"
    );
}

#[test]
fn keeps_the_standard_width_in_range() {
    let width = |value: Value| normalize_tab_settings(&json!({"tab_standard_width": value}));

    assert_eq!(width(json!(150.4)).tab_standard_width, 150);
    assert_eq!(width(json!(20)).tab_standard_width, 80);
    assert_eq!(width(json!(999)).tab_standard_width, 300);
}

#[test]
fn shows_the_hover_tooltip_only_when_turned_on() {
    assert!(normalize_tab_settings(&json!({"tab_hover_tooltip": true})).tab_hover_tooltip);
    assert!(!normalize_tab_settings(&json!({"tab_hover_tooltip": "yes"})).tab_hover_tooltip);
}

#[test]
fn fills_in_the_defaults() {
    assert_eq!(
        serde_json::to_value(normalize_tab_settings(&json!({}))).unwrap(),
        serde_json::to_value(default_tab_settings()).unwrap()
    );
    assert_eq!(DEFAULT_TAB_STANDARD_WIDTH, 120);
}

#[test]
fn shows_headers_unless_turned_off_and_keeps_the_fade_in_range() {
    assert!(
        !normalize_tab_settings(&json!({"panes": {"show_headers": false}}))
            .panes
            .show_headers
    );
    assert!(
        normalize_tab_settings(&json!({"panes": {"show_headers": "no"}}))
            .panes
            .show_headers
    );

    let dim = |value: Value| {
        normalize_tab_settings(&json!({"panes": {"dim_inactive_percent": value}}))
            .panes
            .dim_inactive_percent
    };
    assert_eq!(dim(json!(30.4)), 30);
    assert_eq!(dim(json!(-5)), 0);
    assert_eq!(dim(json!(200)), 60);
    assert_eq!(dim(json!("30")), 0);
}

#[test]
fn drops_saved_layouts_that_are_broken() {
    let panes = normalize_tab_settings(&json!({
        "panes": {"templates": [{"id": "a", "name": "x", "root": {"type": "pane"}}]}
    }))
    .panes;

    assert!(panes.templates.is_empty());
}

#[test]
fn oracle_matches_the_default_settings_and_pane_options() {
    let oracle = oracle();
    let values = &oracle["values"];

    assert_eq!(
        serde_json::to_value(default_tab_settings()).unwrap(),
        values["tabSettings.defaults"]
    );

    let options: Vec<u8> = values["tabSettings.inactivePaneDimOptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(options, INACTIVE_PANE_DIM_OPTIONS);
}

#[test]
fn a_non_object_config_falls_back_to_the_defaults() {
    assert_eq!(normalize_tab_settings(&Value::Null), default_tab_settings());
    assert_eq!(
        normalize_tab_settings(&json!("standard")),
        default_tab_settings()
    );
    assert_eq!(
        normalize_tab_settings(&json!({"panes": []})).panes,
        default_tab_settings().panes
    );
}

#[test]
fn rounds_fractions_like_javascript() {
    assert_eq!(
        normalize_tab_settings(&json!({"tab_standard_width": 79.5})).tab_standard_width,
        80
    );
    assert_eq!(
        normalize_tab_settings(&json!({"panes": {"dim_inactive_percent": 60.6}}))
            .panes
            .dim_inactive_percent,
        60
    );
}
