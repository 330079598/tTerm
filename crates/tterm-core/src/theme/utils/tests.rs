use super::*;
use crate::theme::derivation::{TerminalScheme, derive_theme_colors, terminal_palette_from_scheme};
use crate::theme::types::CustomTheme;
use serde_json::Value;

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

#[test]
fn creating_a_theme_from_a_preset_matches_the_oracle() {
    let oracle = oracle();
    let now = oracle["utils"]["now"].as_i64().expect("the fixed time");
    let cases = oracle["utils"]["createCustomThemeFromPreset"]
        .as_array()
        .expect("an array");
    let inputs = [
        ("ocean", "Mine", None, "custom-1"),
        ("sunset", "Warm", Some("A description"), "custom-2"),
        ("nope", "Unknown", None, "custom-3"),
    ];
    for (index, (preset, name, description, id)) in inputs.into_iter().enumerate() {
        let actual =
            create_custom_theme_from_preset(preset, name, description, now).with_id(id.to_string());
        assert_eq!(
            serde_json::to_value(&actual).expect("the custom theme serializes"),
            cases[index]["value"],
            "{}",
            cases[index]["name"]
        );
    }
}

#[test]
fn unknown_presets_keep_the_passed_base_theme() {
    let new_theme = create_custom_theme_from_preset("nope", "Unknown", None, 1);
    assert_eq!(new_theme.base_theme, OptionalText::Value("nope".into()));
    assert_eq!(
        new_theme.colors,
        crate::theme::definitions::PRESET_THEMES[0].colors
    );
}

#[test]
fn created_custom_theme_json_matches_the_oracle_byte_for_byte() {
    let oracle = oracle();
    let now = oracle["utils"]["now"].as_i64().expect("the fixed time");
    let actual =
        create_custom_theme_from_preset("ocean", "Mine", None, now).with_id("custom-1".into());
    assert_eq!(
        serde_json::to_string(&actual).expect("the custom theme serializes"),
        oracle["serialization"]["customThemeJson"]
            .as_str()
            .expect("custom theme JSON")
    );
}

#[test]
fn imported_custom_theme_json_matches_the_oracle_byte_for_byte() {
    let oracle = oracle();
    let now = oracle["utils"]["now"].as_i64().expect("the fixed time");
    let scheme = TerminalScheme {
        name: Some("Dracula".into()),
        background: Some("#282a36".into()),
        foreground: Some("#f8f8f2".into()),
        palette: [
            "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
            "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
        ]
        .into_iter()
        .map(|color| Some(color.to_string()))
        .collect(),
        cursor: Some("#f8f8f2".into()),
        cursor_text: Some("#282a36".into()),
        selection_background: Some("#44475a".into()),
        selection_foreground: Some("#ffffff".into()),
    };
    let terminal = terminal_palette_from_scheme(&scheme);
    let colors = derive_theme_colors(&terminal);
    let actual = CustomTheme::imported(
        "custom-2".into(),
        "Dracula".into(),
        OptionalText::Value("Imported from iTerm2".into()),
        colors,
        terminal,
        now,
        now,
    );
    assert_eq!(
        serde_json::to_string(&actual).expect("the imported theme serializes"),
        oracle["serialization"]["importedThemeJson"]
            .as_str()
            .expect("imported theme JSON")
    );
}
