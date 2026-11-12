use super::*;
use serde_json::Value;

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

fn check_case(name: &str) {
    let oracle = oracle();
    let case = oracle["import"]["cases"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("the oracle has a case named {name}"));
    let text = case["text"].as_str().expect("the case text");
    let file_name = case["fileName"].as_str().expect("the file name");
    match parse_theme_file(text, file_name) {
        Ok(imported) => {
            assert!(
                case["ok"].as_bool().expect("a boolean"),
                "{name} should have been read"
            );
            assert_eq!(
                serde_json::to_value(imported).expect("the import serializes"),
                case["value"],
                "{name}"
            );
        }
        Err(error) => {
            assert!(
                !case["ok"].as_bool().expect("a boolean"),
                "{name} should have failed"
            );
            assert_eq!(error.reason(), case["error"]["reason"], "{name}");
            let missing = case["error"]["missing"]
                .as_array()
                .map(|missing| {
                    missing
                        .iter()
                        .map(|value| value.as_str().expect("a color name").to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            assert_eq!(error.missing(), missing.as_slice(), "{name}");
            assert_eq!(
                error.to_string(),
                case["error"]["message"].as_str().expect("a message"),
                "{name}"
            );
        }
    }
}

#[test]
fn reads_a_ghostty_theme() {
    check_case("ghostty from the TypeScript test");
}

#[test]
fn reads_a_kitty_theme_with_its_name() {
    check_case("kitty from the TypeScript test");
}

#[test]
fn reads_an_alacritty_toml_theme() {
    check_case("alacritty TOML from the TypeScript test");
}

#[test]
fn reads_an_older_alacritty_yaml_theme() {
    check_case("alacritty YAML from the TypeScript test");
}

#[test]
fn reads_an_iterm2_color_preset() {
    check_case("iterm from the TypeScript test");
}

#[test]
fn iterm_requires_each_component_to_close_immediately_after_its_number() {
    check_case("iterm skips a cursor component followed by an XML comment");
    check_case("iterm skips an ANSI component followed by an empty element");
}

#[test]
fn reads_the_schemes_of_a_windows_terminal_settings_file() {
    check_case("windows terminal settings from the TypeScript test");
}

#[test]
fn names_the_colors_a_file_lacks() {
    check_case("ghostty missing colors");
}

#[test]
fn rejects_text_it_cannot_place() {
    check_case("unknown format");
}

#[test]
fn every_oracle_import_case_matches() {
    let oracle = oracle();
    for case in oracle["import"]["cases"].as_array().expect("an array") {
        check_case(case["name"].as_str().expect("a case name"));
    }
}

#[test]
fn format_specific_keys_are_not_accepted_by_the_wrong_reader() {
    let cases = oracle();
    let ghostty = cases["import"]["cases"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|case| case["name"] == "ghostty from the TypeScript test")
        .expect("the ghostty case");
    let text = format!(
        "{}\ncursor = #ffffff\ncursor_text_color = #000000",
        ghostty["text"].as_str().expect("the text")
    );
    let imported = parse_theme_file(&text, "ignored.conf").expect("a Ghostty theme");
    assert_eq!(imported.schemes[0].cursor_text, None);
    let kitty = cases["import"]["cases"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|case| case["name"] == "kitty from the TypeScript test")
        .expect("the kitty case");
    let text = format!(
        "{}\ncursor-color #ffffff\ncursor-text #000000",
        kitty["text"].as_str().expect("the text")
    );
    let imported = parse_theme_file(&text, "ignored.conf").expect("a kitty theme");
    assert_eq!(imported.schemes[0].cursor, None);
    assert_eq!(imported.schemes[0].cursor_text, Some("#000000".into()));
}
