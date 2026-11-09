//! The constants are generated from `tokens.json`, so these tests compare them
//! back to it: they catch a hand edit on either side, and they prove that a
//! regeneration kept every theme, colour and font size the stylesheet declares.

use serde_json::Value;

use crate::tokens::color::Hsl;
use crate::tokens::font_size::FontSize;
use crate::tokens::palette::{RADIUS_REM, THEMES};
use crate::tokens::type_scale::FONT_SIZES;

const TOKENS: &str = include_str!("../../../../src/styles/tokens.json");

fn tokens() -> Value {
    serde_json::from_str(TOKENS).expect("tokens.json is valid JSON")
}

fn number(value: &Value) -> f32 {
    value.as_f64().expect("a number in tokens.json") as f32
}

/// The CSS states a colour as `hue saturation% lightness%`.
fn hsl(value: &Value) -> Hsl {
    let text = value.as_str().expect("a colour is written as text");
    let parts: Vec<&str> = text.split(' ').collect();
    assert_eq!(
        parts.len(),
        3,
        "{text} is not a hue, saturation and lightness"
    );

    let component = |part: &str| -> f32 {
        part.trim_end_matches('%')
            .parse()
            .unwrap_or_else(|_| panic!("{text} has a component that is not a number"))
    };
    Hsl::new(
        component(parts[0]),
        component(parts[1]),
        component(parts[2]),
    )
}

#[test]
fn radius_matches_tokens_json() {
    assert_eq!(RADIUS_REM, number(&tokens()["radiusRem"]));
}

#[test]
fn font_sizes_match_tokens_json() {
    let tokens = tokens();
    let expected = tokens["fontSizes"]
        .as_object()
        .expect("fontSizes is an object");
    assert_eq!(FONT_SIZES.len(), expected.len());

    for (name, size) in FONT_SIZES {
        let entry = expected
            .get(name)
            .unwrap_or_else(|| panic!("tokens.json declares no --text-{name}"));
        let declared = FontSize {
            rem: number(&entry["rem"]),
            line_height: number(&entry["lineHeight"]),
        };
        assert_eq!(size, declared, "--text-{name}");
    }
}

#[test]
fn theme_colours_match_tokens_json() {
    let tokens = tokens();
    let expected = tokens["themes"].as_array().expect("themes is an array");
    assert_eq!(THEMES.len(), expected.len());

    for (theme, declared) in THEMES.iter().zip(expected) {
        assert_eq!(
            theme.id,
            declared["name"].as_str().expect("a theme has a name")
        );
        let colours = declared["colors"].as_object().expect("a theme has colours");
        for (name, value) in colours {
            assert_eq!(
                theme.colors.get(name),
                Some(hsl(value)),
                "{} --{name}",
                theme.id
            );
        }
    }
}

#[test]
fn the_first_theme_is_the_default_one() {
    let tokens = tokens();
    let first = &tokens["themes"][0];
    assert_eq!(first["selector"].as_str(), Some(":root"));
    assert_eq!(
        THEMES[0].id,
        first["name"].as_str().expect("a theme has a name")
    );
}

#[test]
fn theme_ids_are_unique() {
    let mut ids: Vec<&str> = THEMES.iter().map(|theme| theme.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), THEMES.len());
}

#[test]
fn dark_variant_matches_css_theme_selector_rule() {
    let stylesheet = include_str!("../../../../src/styles/themes.css");
    assert!(stylesheet.contains(
        "@custom-variant dark (&:where(:not([data-theme=\"light\"], [data-theme=\"light\"] *)))"
    ));
    for theme in THEMES {
        assert_eq!(
            theme.uses_dark_variant(),
            theme.id != "light",
            "{}",
            theme.id
        );
    }
    assert_eq!(
        THEMES
            .iter()
            .filter(|theme| theme.uses_dark_variant())
            .count(),
        5
    );
    assert!(!THEMES[1].uses_dark_variant());
    let bright_custom = crate::tokens::theme::Theme {
        id: "ocean",
        colors: THEMES[1].colors,
    };
    assert!(bright_custom.uses_dark_variant());
}

#[test]
fn a_colour_name_is_looked_up_without_its_dashes() {
    assert_eq!(
        THEMES[0].colors.get("background"),
        Some(THEMES[0].colors.background)
    );
    assert_eq!(THEMES[0].colors.get("--background"), None);
    assert_eq!(THEMES[0].colors.get("outline"), None);
}
