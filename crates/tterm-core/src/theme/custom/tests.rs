use super::*;
use crate::theme::definitions::PRESET_THEMES;
use serde_json::{Value, json};

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

fn now() -> i64 {
    oracle()["utils"]["now"].as_i64().expect("the fixed time")
}

#[test]
fn preset_ids_are_the_six_built_ins_only() {
    let oracle = oracle();
    for case in oracle["custom"]["isPresetThemeId"]
        .as_array()
        .expect("an array")
    {
        let id = case["id"].as_str().expect("an id");
        assert_eq!(
            is_preset_theme_id(id),
            case["value"].as_bool().expect("a boolean"),
            "{id:?}"
        );
    }
}

#[test]
fn merging_a_preset_override_matches_the_oracle() {
    let oracle = oracle();
    let preset = &PRESET_THEMES[2];
    let cases = oracle["custom"]["merge"].as_array().expect("an array");
    assert_eq!(
        serde_json::to_value(merge_preset_theme_with_override(preset, None))
            .expect("the preset serializes"),
        cases[0]["value"]
    );
    let override_value = cases[1]["value"].clone();
    let override_theme = normalize_custom_themes(&[override_value], now())
        .into_iter()
        .next()
        .expect("one custom theme");
    assert_eq!(
        serde_json::to_value(merge_preset_theme_with_override(
            preset,
            Some(&override_theme)
        ))
        .expect("the merged preset serializes"),
        cases[1]["value"]
    );
}

#[test]
fn normalized_custom_themes_match_the_oracle() {
    let oracle = oracle();
    for case in oracle["custom"]["normalize"].as_array().expect("an array") {
        let themes = case["themes"].as_array().expect("themes");
        let actual = serde_json::to_value(normalize_custom_themes(themes, now()))
            .expect("the normalized themes serialize");
        assert_eq!(actual, case["value"], "{}", case["name"]);
    }
}

#[test]
fn normalized_custom_theme_json_matches_the_oracle_byte_for_byte() {
    let oracle = oracle();
    for (index, key) in [
        "normalizedMixedJson",
        "normalizedEdgeJson",
        "normalizedStringJson",
    ]
    .into_iter()
    .enumerate()
    {
        let themes = oracle["custom"]["normalize"][index]["themes"]
            .as_array()
            .expect("themes");
        let actual = serde_json::to_string(&normalize_custom_themes(themes, now()))
            .expect("the normalized themes serialize");
        assert_eq!(
            actual,
            oracle["serialization"][key]
                .as_str()
                .expect("normalized theme JSON"),
            "{key}"
        );
    }
}

#[test]
fn normalization_keeps_missing_null_and_unknown_members_apart() {
    let themes = normalize_custom_themes(
        &[json!({
            "id": "edge",
            "name": "Edge",
            "description": null,
            "baseTheme": null,
            "colors": {
                "background": null,
                "unknownColor": {"nested": true}
            },
            "terminal": {
                "background": null,
                "extraTerminal": 7
            },
            "createdAt": 0,
            "updatedAt": null
        })],
        now(),
    );
    assert_eq!(themes.len(), 1);
    let theme = &themes[0];
    assert_eq!(theme.description, OptionalText::Null);
    assert_eq!(theme.base_theme, OptionalText::Null);
    assert_eq!(theme.created_at, 0);
    assert_eq!(theme.updated_at, now());
    let serialized = serde_json::to_value(theme).expect("the theme serializes");
    assert_eq!(serialized["description"], Value::Null);
    assert_eq!(serialized["baseTheme"], Value::Null);
    assert_eq!(serialized["colors"]["background"], Value::Null);
    assert_eq!(
        serialized["colors"]["unknownColor"],
        json!({"nested": true})
    );
    assert_eq!(serialized["terminal"]["background"], Value::Null);
    assert_eq!(serialized["terminal"]["extraTerminal"], 7);
    assert_eq!(serialized["isCustom"], true);
}

#[test]
fn a_missing_terminal_uses_the_base_theme_palette() {
    let themes = normalize_custom_themes(
        &[json!({
            "id": "missing-terminal",
            "name": "Missing terminal",
            "colors": {},
            "baseTheme": "light"
        })],
        now(),
    );
    assert_eq!(&themes[0].terminal, &PRESET_THEMES[1].terminal);
    let serialized = serde_json::to_value(&themes[0]).expect("the theme serializes");
    assert_eq!(
        serialized["terminal"],
        serde_json::to_value(&PRESET_THEMES[1].terminal).expect("the palette serializes")
    );
}

#[test]
fn empty_values_are_rejected_the_way_truthiness_rejects_them() {
    let invalid = [
        json!(null),
        json!("text"),
        json!(3),
        json!([]),
        json!({}),
        json!({"id": "", "name": "No id", "colors": {}}),
        json!({"id": "a", "name": "", "colors": {}}),
        json!({"id": "b", "name": "B"}),
        json!({"id": "c", "name": "C", "colors": null}),
        json!({"id": "d", "name": "D", "colors": ""}),
    ];
    assert!(normalize_custom_themes(&invalid, now()).is_empty());
}

#[test]
fn an_empty_colors_array_is_truthy_and_keeps_the_fallback_colors() {
    let themes = normalize_custom_themes(
        &[json!({
            "id": "array-colors",
            "name": "Array colors",
            "colors": []
        })],
        now(),
    );
    assert_eq!(themes.len(), 1);
    assert_eq!(themes[0].colors, PRESET_THEMES[0].colors);
}

#[test]
fn empty_string_ids_and_names_are_dropped_but_zero_timestamps_are_kept() {
    let themes = normalize_custom_themes(
        &[json!({
            "id": "kept",
            "name": "Kept",
            "colors": {},
            "createdAt": 0,
            "updatedAt": 0
        })],
        now(),
    );
    assert_eq!(themes.len(), 1);
    assert_eq!(themes[0].created_at, 0);
    assert_eq!(themes[0].updated_at, 0);
}
