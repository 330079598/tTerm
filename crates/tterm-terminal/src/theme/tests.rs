use super::*;
use crate::theme::js::{js_number, js_number_to_string, js_round, js_trim};
use serde_json::Value;

const ORACLE_JSON: &str =
    include_str!("../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");
const CSS_ORACLE_JSON: &str =
    include_str!("../../../../docs/gpui-migration/reports/P1-11f-evidence/css-colors.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

fn float(value: &Value) -> f64 {
    value.as_f64().expect("a JSON number")
}

fn rgba(value: &Value) -> Option<Rgba> {
    let channels = value.as_array()?;
    Some(Rgba::new(
        channels[0].as_u64()? as u8,
        channels[1].as_u64()? as u8,
        channels[2].as_u64()? as u8,
        channels[3].as_u64()? as u8,
    ))
}

fn set_theme_color(colors: &mut ThemeColors, key: &str, value: &str) {
    let field = THEME_COLOR_KEYS
        .iter()
        .position(|candidate| *candidate == key)
        .expect("a known theme color");
    let value = value.to_string();
    match field {
        0 => colors.background = value,
        1 => colors.foreground = value,
        2 => colors.card = value,
        3 => colors.card_foreground = value,
        4 => colors.popover = value,
        5 => colors.popover_foreground = value,
        6 => colors.primary = value,
        7 => colors.primary_foreground = value,
        8 => colors.secondary = value,
        9 => colors.secondary_foreground = value,
        10 => colors.muted = value,
        11 => colors.muted_foreground = value,
        12 => colors.accent = value,
        13 => colors.accent_foreground = value,
        14 => colors.destructive = value,
        15 => colors.destructive_foreground = value,
        16 => colors.success = value,
        17 => colors.success_foreground = value,
        18 => colors.warning = value,
        19 => colors.warning_foreground = value,
        20 => colors.border = value,
        21 => colors.input = value,
        22 => colors.ring = value,
        23 => colors.tab_background = value,
        24 => colors.tab_active = value,
        25 => colors.tab_active_border = value,
        26 => colors.tab_hover = value,
        27 => colors.titlebar = value,
        _ => unreachable!("the field index came from THEME_COLOR_KEYS"),
    }
}

#[test]
fn numbers_format_like_javascript() {
    assert_eq!(js_number_to_string(1e-7), "1e-7");
    assert_eq!(js_number_to_string(0.000001), "0.000001");
    assert_eq!(js_number_to_string(1e21), "1e+21");
    assert_eq!(
        js_number_to_string(1.000_000_224_848_918_2e-7),
        "1.0000002248489182e-7"
    );
    assert_eq!(js_number_to_string(-0.0), "0");
    assert_eq!(
        js_number_to_string(222.200_000_000_000_05),
        "222.20000000000005"
    );
}

#[test]
fn color_to_hex_matches_color_utils() {
    let oracle = oracle();
    for case in oracle["colors"]["colorToHex"].as_array().expect("an array") {
        let input = case["value"].as_str().expect("a string");
        let expected = case["hex"].as_str();
        assert_eq!(color_to_hex(input).as_deref(), expected, "{input:?}");
    }
}

#[test]
fn hex_to_rgb_matches_color_utils() {
    let oracle = oracle();
    for case in oracle["colors"]["hexToRgb"].as_array().expect("an array") {
        let input = case["value"].as_str().expect("a string");
        let actual = hex_to_rgb(input).map(|color| {
            serde_json::json!({
                "r": color.r,
                "g": color.g,
                "b": color.b,
            })
        });
        let expected = if case["rgb"].is_null() {
            None
        } else {
            Some(case["rgb"].clone())
        };
        assert_eq!(actual, expected, "{input:?}");
    }
}

#[test]
fn rgb_to_hex_matches_color_utils() {
    let oracle = oracle();
    for case in oracle["colors"]["rgbToHex"].as_array().expect("an array") {
        let input = case["input"].as_array().expect("three channels");
        let actual = rgb_to_hex(float(&input[0]), float(&input[1]), float(&input[2]));
        assert_eq!(actual, case["hex"].as_str().expect("hex"), "{input:?}");
    }
}

#[test]
fn hsl_to_rgb_matches_color_utils() {
    let oracle = oracle();
    for case in oracle["colors"]["hslToRgb"].as_array().expect("an array") {
        let input = case["input"].as_array().expect("three channels");
        let rgb = hsl_to_rgb(float(&input[0]), float(&input[1]), float(&input[2]));
        assert_eq!(
            rgb_to_hex(rgb.r, rgb.g, rgb.b),
            case["hex"].as_str().expect("hex"),
            "{input:?}"
        );
    }
}

#[test]
fn relative_luminance_and_contrast_match_color_utils() {
    let oracle = oracle();
    for case in oracle["colors"]["relativeLuminance"]
        .as_array()
        .expect("an array")
    {
        let input = case["value"].as_str().expect("a string");
        match (relative_luminance(input), case["luminance"].as_f64()) {
            (Some(actual), Some(expected)) => {
                assert!(
                    (actual - expected).abs() <= 1e-15,
                    "{input:?}: {actual} != {expected}"
                );
            }
            (None, None) => {}
            (actual, expected) => panic!("{input:?}: {actual:?} != {expected:?}"),
        }
    }
    for case in oracle["colors"]["contrastRatio"]
        .as_array()
        .expect("an array")
    {
        let first = case["first"].as_str().expect("a string");
        let second = case["second"].as_str().expect("a string");
        match (contrast_ratio(first, second), case["ratio"].as_f64()) {
            (Some(actual), Some(expected)) => {
                assert!(
                    (actual - expected).abs() <= 1e-15,
                    "{first:?} {second:?}: {actual} != {expected}"
                );
            }
            (None, None) => {}
            (actual, expected) => panic!("{first:?} {second:?}: {actual:?} != {expected:?}"),
        }
    }
}

#[test]
fn parse_hsl_matches_terminal_palette() {
    let oracle = oracle();
    for case in oracle["terminalPalette"]["parseHsl"]
        .as_array()
        .expect("an array")
    {
        let input = case["value"].as_str().expect("a string");
        match (parse_hsl(input), case["result"].as_object()) {
            (Some(actual), Some(expected)) => {
                assert_eq!(actual.h, float(&expected["h"]), "{input:?}");
                assert_eq!(actual.s, float(&expected["s"]), "{input:?}");
                assert_eq!(actual.l, float(&expected["l"]), "{input:?}");
            }
            (None, None) => {}
            (actual, expected) => panic!("{input:?}: {actual:?} != {expected:?}"),
        }
    }
}

#[test]
fn palette_derivation_matches_the_generated_oracle_palettes() {
    let oracle = oracle();
    for case in oracle["terminalPalette"]["generated"]
        .as_array()
        .expect("an array")
    {
        let mut colors = ThemeColors::dark_default();
        for (key, value) in case["colors"].as_object().expect("color tokens") {
            set_theme_color(&mut colors, key, value.as_str().expect("a token string"));
        }
        let actual = serde_json::to_value(PaletteText::from_theme_colors(&colors))
            .expect("the palette serializes");
        assert_eq!(
            actual,
            case["palette"],
            "{}",
            case["name"].as_str().expect("a case name")
        );
    }
}

#[test]
fn window_blur_and_transparency_match_terminal_palette() {
    let oracle = oracle();
    for case in oracle["terminalPalette"]["withWindowBlur"]
        .as_array()
        .expect("an array")
    {
        let mut palette = PaletteText::default();
        if let Some(background) = case["input"]["background"].as_str() {
            palette.background = background.to_string();
        }
        let actual = palette.with_window_blur(case["blur"].as_bool().expect("a boolean"));
        if let Some(expected) = case["palette"]["background"].as_str() {
            assert_eq!(actual.background, expected);
        } else {
            assert_eq!(actual.background, palette.background);
        }
        assert_eq!(actual.foreground, palette.foreground);
    }
    for case in oracle["terminalPalette"]["isTransparent"]
        .as_array()
        .expect("an array")
    {
        let mut palette = PaletteText::default();
        if let Some(background) = case["palette"]["background"].as_str() {
            palette.background = background.to_string();
        }
        assert_eq!(
            palette.is_transparent(),
            case["value"].as_bool().expect("a boolean"),
            "{}",
            case["palette"]
        );
    }
}

#[test]
fn palette_text_round_trips_unknown_terminal_keys_and_nulls() {
    let value = serde_json::json!({
        "background": "#000000",
        "selectionForeground": null,
        "unknownTerminal": "kept"
    });
    let palette: PaletteText = serde_json::from_value(value.clone()).expect("a palette");
    assert_eq!(palette.selection_foreground, OptionalText::Null);
    assert_eq!(
        palette.extra.get("unknownTerminal"),
        Some(&Value::String("kept".into()))
    );
    let serialized = serde_json::to_value(palette).expect("the palette serializes");
    assert_eq!(serialized["background"], "#000000");
    assert_eq!(serialized["selectionForeground"], Value::Null);
    assert_eq!(serialized["unknownTerminal"], "kept");
}

#[test]
fn null_optional_palette_fields_serialize_like_the_frontend() {
    let oracle = oracle();
    let mut palette: PaletteText =
        serde_json::from_value(oracle["terminalPalette"]["defaultPalette"].clone())
            .expect("the default palette");
    palette.cursor_accent = OptionalText::Null;
    palette.selection_foreground = OptionalText::Null;
    assert_eq!(
        serde_json::to_string(&palette).expect("the palette serializes"),
        oracle["serialization"]["nullTerminalJson"]
            .as_str()
            .expect("null palette JSON")
    );
}

#[test]
fn palette_colors_match_xterm() {
    let oracle: Value =
        serde_json::from_str(CSS_ORACLE_JSON).expect("the CSS oracle JSON is valid");
    for case in oracle["xterm"].as_array().expect("an array") {
        let value = case["value"].as_str().expect("a color string");
        let expected = rgba(&case["rgba"]).expect("four channels");
        let palette = PaletteText {
            background: value.to_string(),
            ..PaletteText::default()
        };
        assert_eq!(
            palette.to_terminal_palette().background,
            expected,
            "{value}"
        );
    }
}

#[test]
fn hsl_alpha_is_rejected_the_way_xterm_rejects_canvas_transparency() {
    let oracle: Value =
        serde_json::from_str(CSS_ORACLE_JSON).expect("the CSS oracle JSON is valid");
    for case in oracle["themePalette"].as_array().expect("an array") {
        let value = case["value"].as_str().expect("a color string");
        let expected = rgba(&case["rgba"]).expect("four channels");
        let palette = PaletteText {
            background: value.to_string(),
            ..PaletteText::default()
        };
        assert_eq!(
            palette.to_terminal_palette().background,
            expected,
            "{value}"
        );
    }
}

fn xterm_baseline_palette() -> PaletteText {
    PaletteText {
        background: "#101010".into(),
        foreground: "#eeeeee".into(),
        cursor: "#eeeeee".into(),
        cursor_accent: OptionalText::Value("#101010".into()),
        selection_background: "rgba(1, 2, 3, 0.5)".into(),
        selection_foreground: OptionalText::Value("#eeeeee".into()),
        black: "#111111".into(),
        bright_white: "#ffffff".into(),
        ..PaletteText::default()
    }
}

fn set_palette_field(palette: &mut PaletteText, role: &str, value: &str) {
    let value = value.to_string();
    match role {
        "background" => palette.background = value,
        "foreground" => palette.foreground = value,
        "cursor" => palette.cursor = value,
        "cursorAccent" => palette.cursor_accent = OptionalText::Value(value),
        "selectionBackground" => palette.selection_background = value,
        "selectionForeground" => palette.selection_foreground = OptionalText::Value(value),
        "black" => palette.black = value,
        "brightWhite" => palette.bright_white = value,
        _ => panic!("the oracle names a known field: {role}"),
    }
}

fn palette_field(palette: &TerminalPalette, role: &str) -> Option<Rgba> {
    match role {
        "background" => Some(palette.background),
        "foreground" => Some(palette.foreground),
        "cursor" => Some(palette.cursor),
        "cursorAccent" => palette.cursor_accent,
        "selectionBackground" => Some(palette.selection_background),
        "selectionForeground" => palette.selection_foreground,
        "black" => Some(palette.black),
        "brightWhite" => Some(palette.bright_white),
        _ => panic!("the oracle names a known field: {role}"),
    }
}

#[test]
fn each_xterm_field_resolves_every_css_color_form() {
    let oracle: Value =
        serde_json::from_str(CSS_ORACLE_JSON).expect("the CSS oracle JSON is valid");
    for case in oracle["fields"].as_array().expect("an array") {
        let role = case["role"].as_str().expect("a role");
        let value = case["value"].as_str().expect("a color string");
        let expected = rgba(&case["rgba"]);
        let mut palette = xterm_baseline_palette();
        set_palette_field(&mut palette, role, value);
        assert_eq!(
            palette_field(&palette.to_terminal_palette(), role),
            expected,
            "{role} {value}"
        );
    }
}

#[test]
fn a_blurred_background_keeps_its_rgb_and_zero_alpha() {
    let palette: PaletteText =
        serde_json::from_value(oracle()["terminalPalette"]["defaultPalette"].clone())
            .expect("the default palette");
    let palette = palette.with_window_blur(true);
    assert_eq!(
        palette.to_terminal_palette().background,
        Rgba::new(0x1b, 0x1d, 0x23, 0)
    );
}

#[test]
fn trim_matches_javascript_whitespace() {
    let oracle = oracle();
    for case in oracle["helpers"]["jsTrim"].as_array().expect("an array") {
        let input = case["value"].as_str().expect("a string");
        assert_eq!(
            js_trim(input),
            case["trimmed"].as_str().expect("the trimmed value"),
            "{input:?}"
        );
    }
}

#[test]
fn number_conversion_matches_javascript() {
    let oracle = oracle();
    for case in oracle["helpers"]["jsNumber"].as_array().expect("an array") {
        let input = case["value"].as_str().expect("a string");
        assert_eq!(
            js_number_to_string(js_number(input)),
            case["number"].as_str().expect("the converted number"),
            "{input:?}"
        );
    }
}

#[test]
fn rounding_matches_javascript() {
    let oracle = oracle();
    for case in oracle["helpers"]["jsRound"].as_array().expect("an array") {
        let input = case["value"].as_str().expect("a number string");
        let value = match input {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            _ => input.parse::<f64>().expect("a finite number"),
        };
        assert_eq!(
            js_number_to_string(js_round(value)),
            case["rounded"].as_str().expect("the rounded number"),
            "{input:?}"
        );
    }
}

#[test]
fn number_to_string_matches_javascript() {
    let oracle = oracle();
    for case in oracle["helpers"]["jsNumberToString"]
        .as_array()
        .expect("an array")
    {
        let input = case["input"].as_str().expect("a number string");
        let value = input.parse::<f64>().expect("a number");
        assert_eq!(
            js_number_to_string(value),
            case["string"].as_str().expect("the formatted number"),
            "{value:?}"
        );
    }
}
