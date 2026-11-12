use super::*;
use serde_json::Value;

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

fn dracula() -> TerminalScheme {
    TerminalScheme {
        name: OptionalText::Value("Dracula".into()),
        background: OptionalText::Value("#282a36".into()),
        foreground: OptionalText::Value("#f8f8f2".into()),
        palette: [
            "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
            "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
        ]
        .into_iter()
        .map(|color| Some(color.to_string()))
        .collect(),
        cursor: OptionalText::Value("#f8f8f2".into()),
        cursor_text: OptionalText::Value("#282a36".into()),
        selection_background: OptionalText::Value("#44475a".into()),
        selection_foreground: OptionalText::Value("#ffffff".into()),
    }
}

#[test]
fn maps_a_scheme_onto_the_terminal_palette() {
    let palette = terminal_palette_from_scheme(&dracula());
    assert_eq!(palette.background, "#282a36");
    assert_eq!(palette.foreground, "#f8f8f2");
    assert_eq!(palette.cursor, "#f8f8f2");
    assert_eq!(palette.cursor_accent, OptionalText::Value("#282a36".into()));
    assert_eq!(palette.selection_background, "#44475a");
    assert_eq!(
        palette.selection_foreground,
        OptionalText::Value("#ffffff".into())
    );
    assert_eq!(palette.black, "#21222c");
    assert_eq!(palette.blue, "#bd93f9");
    assert_eq!(palette.bright_white, "#ffffff");
}

#[test]
fn fills_in_the_colors_a_scheme_leaves_out() {
    let palette = terminal_palette_from_scheme(&TerminalScheme {
        background: OptionalText::Value("#000".into()),
        foreground: OptionalText::Value("#fff".into()),
        palette: dracula().palette,
        ..TerminalScheme::default()
    });
    assert_eq!(palette.cursor, "#ffffff");
    assert_eq!(palette.selection_background, "rgba(255, 255, 255, 0.25)");
    assert_eq!(palette.selection_foreground, OptionalText::Missing);
}

#[test]
fn builds_the_interface_from_the_palettes_own_colors() {
    let colors = derive_theme_colors(&terminal_palette_from_scheme(&dracula()));
    assert_eq!(color_to_hex(&colors.background).as_deref(), Some("#282a36"));
    assert_eq!(
        color_to_hex(&colors.tab_background).as_deref(),
        Some("#282a36")
    );
    assert_eq!(colors.primary, colors.ring);
    assert_eq!(color_to_hex(&colors.primary).as_deref(), Some("#bd93f9"));
    assert_eq!(
        color_to_hex(&colors.destructive).as_deref(),
        Some("#ff5555")
    );
}

#[test]
fn falls_back_to_another_hue_when_the_blue_is_gray() {
    let mut scheme = dracula();
    scheme.palette[4] = Some("#808080".into());
    scheme.palette[12] = Some("#808080".into());
    scheme.cursor = OptionalText::Value("#ff9900".into());
    let palette = terminal_palette_from_scheme(&scheme);
    assert_eq!(
        color_to_hex(&derive_theme_colors(&palette).primary).as_deref(),
        Some("#ff9900")
    );
}

#[test]
fn scheme_projection_matches_the_oracle() {
    let oracle = oracle();
    for case in oracle["derivation"]["schemes"]
        .as_array()
        .expect("an array")
    {
        assert!(case["ok"].as_bool().expect("a boolean"), "{}", case["name"]);
        let scheme: TerminalScheme =
            serde_json::from_value(case["scheme"].clone()).expect("a terminal scheme");
        let palette = terminal_palette_from_scheme(&scheme);
        assert_eq!(
            serde_json::to_value(&palette).expect("the palette serializes"),
            case["palette"],
            "{}",
            case["name"]
        );
        assert_eq!(
            serde_json::to_value(derive_theme_colors(&palette)).expect("colors serialize"),
            case["colors"],
            "{}",
            case["name"]
        );
        assert_eq!(
            is_dark_palette(&palette),
            case["dark"].as_bool().expect("a boolean"),
            "{}",
            case["name"]
        );
    }
}

#[test]
fn optional_scheme_fields_keep_missing_and_null_distinct() {
    let oracle = oracle();
    let missing: TerminalScheme =
        serde_json::from_value(oracle["derivation"]["schemes"][1]["scheme"].clone())
            .expect("a scheme without optional colors");
    assert_eq!(
        serde_json::to_string(&missing).expect("the scheme serializes"),
        oracle["serialization"]["schemeMissingJson"]
            .as_str()
            .expect("missing scheme JSON")
    );
    let mut value = serde_json::to_value(&missing).expect("the scheme serializes");
    for key in [
        "name",
        "background",
        "foreground",
        "cursor",
        "cursorText",
        "selectionBackground",
        "selectionForeground",
    ] {
        value[key] = Value::Null;
    }
    let with_nulls: TerminalScheme =
        serde_json::from_value(value).expect("a scheme with null optional colors");
    assert_eq!(
        serde_json::to_string(&with_nulls).expect("the scheme serializes"),
        oracle["serialization"]["schemeNullJson"]
            .as_str()
            .expect("null scheme JSON")
    );
}

#[test]
fn derived_interface_colors_match_the_oracle() {
    let oracle = oracle();
    for case in oracle["derivation"]["colors"].as_array().expect("an array") {
        let palette: PaletteText =
            serde_json::from_value(case["palette"].clone()).expect("a terminal palette");
        assert_eq!(
            serde_json::to_value(derive_theme_colors(&palette)).expect("colors serialize"),
            case["colors"],
            "{}",
            case["name"]
        );
    }
}
