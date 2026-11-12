use super::*;
use crate::theme::types::{PresetThemeId, ThemeSlot};
use serde_json::Value;

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

#[test]
fn presets_match_the_typescript_definitions() {
    let oracle = oracle();
    assert_eq!(
        serde_json::to_value(PRESET_THEMES.as_slice()).expect("presets serialize"),
        oracle["definitions"]["presets"]
    );
}

#[test]
fn preset_json_matches_byte_for_byte() {
    let oracle = oracle();
    assert_eq!(
        serde_json::to_string(&PRESET_THEMES[0]).expect("the preset serializes"),
        oracle["definitions"]["presetThemeJson"]
            .as_str()
            .expect("preset JSON")
    );
    assert_eq!(
        serde_json::to_string(PRESET_THEMES.as_slice()).expect("the presets serialize"),
        oracle["definitions"]["presetsJson"]
            .as_str()
            .expect("presets JSON")
    );
    assert_eq!(
        serde_json::to_string(&PRESET_THEMES[0].terminal).expect("the palette serializes"),
        oracle["serialization"]["defaultTerminalJson"]
            .as_str()
            .expect("default palette JSON")
    );
    assert_eq!(
        serde_json::to_string(&PRESET_THEMES[0].terminal.with_window_blur(true))
            .expect("the blurred palette serializes"),
        oracle["serialization"]["blurTerminalJson"]
            .as_str()
            .expect("blurred palette JSON")
    );
}

#[test]
fn preset_lookup_matches_the_typescript_map() {
    let oracle = oracle();
    for case in oracle["definitions"]["getPresetTheme"]
        .as_array()
        .expect("an array")
    {
        let id = case["id"].as_str().expect("an id");
        let actual = get_preset_theme(id)
            .map(|theme| serde_json::to_value(theme).expect("the preset serializes"));
        let expected = if case["value"].is_null() {
            None
        } else {
            Some(case["value"].clone())
        };
        assert_eq!(actual, expected, "{id:?}");
    }
}

#[test]
fn preset_resolution_matches_the_typescript_order() {
    let oracle = oracle();
    let now = oracle["utils"]["now"].as_i64().expect("the fixed time");
    for case in oracle["definitions"]["resolve"]
        .as_array()
        .expect("an array")
    {
        let custom_values = case["custom"].as_array().expect("custom themes");
        let custom = crate::theme::custom::normalize_custom_themes(custom_values, now);
        let id = case["id"].as_str().expect("an id");
        let actual = serde_json::to_value(resolve_theme_definition(id, &custom))
            .expect("the theme serializes");
        assert_eq!(actual, case["value"], "{id:?}");
    }
}

#[test]
fn preset_ids_keep_their_order_and_values() {
    assert_eq!(
        PRESET_THEME_IDS,
        ["default", "light", "ocean", "forest", "sunset", "ubuntu"]
    );
    assert_eq!(
        PRESET_THEMES
            .iter()
            .map(|theme| theme.id.as_str())
            .collect::<Vec<_>>(),
        PRESET_THEME_IDS
    );
    assert_eq!(PresetThemeId::parse("constructor"), None);
    assert_eq!(PresetThemeId::parse("Default"), None);
    assert_eq!(
        [
            PresetThemeId::Default,
            PresetThemeId::Light,
            PresetThemeId::Ocean,
            PresetThemeId::Forest,
            PresetThemeId::Sunset,
            PresetThemeId::Ubuntu,
        ]
        .map(PresetThemeId::as_str),
        PRESET_THEME_IDS
    );
    assert_eq!(PRESET_THEME_MAP.len(), PRESET_THEMES.len());
    for theme in PRESET_THEMES.iter() {
        assert_eq!(
            PRESET_THEME_MAP.get(theme.id.as_str()).copied(),
            Some(theme)
        );
    }
}

#[test]
fn theme_slots_serialize_as_the_frontend_strings() {
    assert_eq!(
        serde_json::to_string(&ThemeSlot::Light).expect("a slot"),
        "\"light\""
    );
    assert_eq!(
        serde_json::to_string(&ThemeSlot::Dark).expect("a slot"),
        "\"dark\""
    );
}
