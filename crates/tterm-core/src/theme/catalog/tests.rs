use super::*;
use crate::theme::derivation::derive_theme_colors;
use crate::theme::types::Theme;
use serde_json::Value;
use tterm_terminal::theme::{ThemeColors, color_to_hex, contrast_ratio, relative_luminance};

const ORACLE_JSON: &str =
    include_str!("../../../../../docs/gpui-migration/reports/P1-11f-evidence/node.json");

fn oracle() -> Value {
    serde_json::from_str(ORACLE_JSON).expect("the oracle JSON is valid")
}

#[test]
fn loads_every_library_theme_under_a_unique_id() {
    let oracle = oracle();
    let themes = load_theme_catalog().expect("the bundled catalog");
    assert!(themes.len() > 600);
    assert_eq!(
        themes.len(),
        oracle["catalog"]["count"].as_u64().expect("a count") as usize
    );
    let unique = themes
        .iter()
        .map(|theme| theme.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), themes.len());
    let theme = themes
        .iter()
        .find(|candidate| candidate.name == "Dracula")
        .expect("Dracula");
    assert!(is_catalog_theme_id(&theme.id));
    assert_eq!(catalog_theme_name(&theme.id), "Dracula");
    assert!(theme.is_dark());
    let light = themes
        .iter()
        .find(|candidate| candidate.name == "Atom One Light")
        .expect("Atom One Light");
    assert!(!light.is_dark());
}

#[test]
fn derives_a_library_themes_interface_colors_once_on_first_use() {
    let themes = load_theme_catalog().expect("the bundled catalog");
    let theme = &themes[0];
    let first = theme.colors();
    let second = theme.colors();
    assert!(std::ptr::eq(first, second));
    assert_eq!(*first, derive_theme_colors(theme.terminal()));
}

#[test]
fn derives_readable_interface_colors_for_every_library_theme() {
    const SLACK: f64 = 0.25;
    let themes = load_theme_catalog().expect("the bundled catalog");
    for theme in themes {
        let colors = theme.colors();
        for key in tterm_terminal::theme::THEME_COLOR_KEYS {
            let value = color(colors, key);
            assert!(is_hsl_token(value), "{} {key}: {value}", theme.name);
        }
        for (text, fill, ratio) in [
            ("foreground", "background", 4.5),
            ("foreground", "card", 4.5),
            ("foreground", "muted", 4.5),
            ("mutedForeground", "card", 4.5),
            ("foreground", "accent", 4.5),
            ("mutedForeground", "background", 4.5),
            ("successForeground", "background", 4.5),
            ("warningForeground", "background", 4.5),
            ("primaryForeground", "primary", 4.5),
            ("destructiveForeground", "destructive", 3.0),
            ("destructive", "background", 3.0),
        ] as [(&str, &str, f64); 11]
        {
            let required = ratio.min(best_contrast(colors, text, fill));
            let actual = contrast(colors, text, fill);
            assert!(
                actual > required - SLACK,
                "{}: {text} on {fill}: {actual}",
                theme.name
            );
        }
    }
}

fn is_hsl_token(value: &str) -> bool {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || !parts[1].ends_with('%') || !parts[2].ends_with('%') {
        return false;
    }
    let number = |part: &str| part.parse::<f64>().is_ok();
    number(parts[0])
        && number(parts[1].trim_end_matches('%'))
        && number(parts[2].trim_end_matches('%'))
}

fn color<'a>(colors: &'a ThemeColors, key: &str) -> &'a str {
    match key {
        "background" => &colors.background,
        "foreground" => &colors.foreground,
        "card" => &colors.card,
        "cardForeground" => &colors.card_foreground,
        "popover" => &colors.popover,
        "popoverForeground" => &colors.popover_foreground,
        "primary" => &colors.primary,
        "primaryForeground" => &colors.primary_foreground,
        "secondary" => &colors.secondary,
        "secondaryForeground" => &colors.secondary_foreground,
        "muted" => &colors.muted,
        "mutedForeground" => &colors.muted_foreground,
        "accent" => &colors.accent,
        "accentForeground" => &colors.accent_foreground,
        "destructive" => &colors.destructive,
        "destructiveForeground" => &colors.destructive_foreground,
        "success" => &colors.success,
        "successForeground" => &colors.success_foreground,
        "warning" => &colors.warning,
        "warningForeground" => &colors.warning_foreground,
        "border" => &colors.border,
        "input" => &colors.input,
        "ring" => &colors.ring,
        "tabBackground" => &colors.tab_background,
        "tabActive" => &colors.tab_active,
        "tabActiveBorder" => &colors.tab_active_border,
        "tabHover" => &colors.tab_hover,
        "titlebar" => &colors.titlebar,
        _ => panic!("the test names a known color: {key}"),
    }
}

fn contrast(colors: &ThemeColors, text: &str, fill: &str) -> f64 {
    contrast_ratio(
        &color_to_hex(color(colors, text)).expect("a text color"),
        &color_to_hex(color(colors, fill)).expect("a fill color"),
    )
    .expect("a contrast ratio")
}

fn best_contrast(colors: &ThemeColors, text: &str, fill: &str) -> f64 {
    let fill_hex = color_to_hex(color(colors, fill)).expect("a fill color");
    let lighter = relative_luminance(&color_to_hex(color(colors, text)).expect("a text color"))
        .expect("text luminance")
        > relative_luminance(&fill_hex).expect("fill luminance");
    contrast_ratio(if lighter { "#ffffff" } else { "#000000" }, &fill_hex)
        .expect("a best contrast ratio")
}

#[test]
fn catalog_ids_names_and_predicates_match_the_oracle() {
    let oracle = oracle();
    assert_eq!(
        THEME_CATALOG_SOURCE,
        "https://github.com/mbadolato/iTerm2-Color-Schemes"
    );
    let themes = load_theme_catalog().expect("the bundled catalog");
    assert_eq!(
        themes.len(),
        oracle["catalog"]["uniqueIds"].as_u64().expect("a count") as usize
    );
    for case in oracle["catalog"]["isCatalogThemeId"]
        .as_array()
        .expect("an array")
    {
        let id = case["id"].as_str().expect("an id");
        assert_eq!(
            is_catalog_theme_id(id),
            case["value"].as_bool().expect("a boolean"),
            "{id:?}"
        );
    }
    for case in oracle["catalog"]["catalogThemeName"]
        .as_array()
        .expect("an array")
    {
        let id = case["id"].as_str().expect("an id");
        assert_eq!(
            catalog_theme_name(id),
            case["value"].as_str().expect("a name"),
            "{id:?}"
        );
    }
    assert!(is_catalog_theme(Some(&Theme::Catalog(themes[0].clone()))));
    assert!(!is_catalog_theme(Some(&Theme::Preset(
        crate::theme::definitions::PRESET_THEMES[0].clone()
    ))));
    assert!(!is_catalog_theme(None));
}

#[test]
fn the_catalog_digest_matches_the_oracle() {
    let oracle = oracle();
    let themes = load_theme_catalog().expect("the bundled catalog");
    let mut input = String::new();
    for (index, theme) in themes.iter().enumerate() {
        if index > 0 {
            input.push('\u{1}');
        }
        input.push_str(
            &[
                theme.id.clone(),
                theme.name.clone(),
                theme.is_dark().to_string(),
            ]
            .join("\0"),
        );
        input.push('\0');
        let colors = theme.colors();
        input.push_str(
            &tterm_terminal::theme::THEME_COLOR_KEYS
                .iter()
                .map(|key| color(colors, key).to_string())
                .collect::<Vec<_>>()
                .join("\0"),
        );
        input.push('\0');
        input.push_str(
            &terminal_values(theme.terminal())
                .into_iter()
                .collect::<Vec<_>>()
                .join("\0"),
        );
    }
    assert_eq!(
        fnv1a64(&input),
        oracle["catalog"]["digest"].as_str().expect("a digest")
    );
}

fn terminal_values(palette: &tterm_terminal::theme::PaletteText) -> Vec<&str> {
    let mut values = vec![
        palette.background.as_str(),
        palette.foreground.as_str(),
        palette.cursor.as_str(),
    ];
    if let Some(value) = palette.cursor_accent.as_str() {
        values.push(value);
    }
    values.push(palette.selection_background.as_str());
    values.extend([
        palette.black.as_str(),
        palette.red.as_str(),
        palette.green.as_str(),
        palette.yellow.as_str(),
        palette.blue.as_str(),
        palette.magenta.as_str(),
        palette.cyan.as_str(),
        palette.white.as_str(),
        palette.bright_black.as_str(),
        palette.bright_red.as_str(),
        palette.bright_green.as_str(),
        palette.bright_yellow.as_str(),
        palette.bright_blue.as_str(),
        palette.bright_magenta.as_str(),
        palette.bright_cyan.as_str(),
        palette.bright_white.as_str(),
    ]);
    if let Some(value) = palette.selection_foreground.as_str() {
        values.push(value);
    }
    values
}

fn fnv1a64(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[test]
fn catalog_theme_serialization_matches_the_oracle() {
    let oracle = oracle();
    let fixed = create_catalog_theme(
        "catalog:Fixed".into(),
        crate::theme::definitions::PRESET_THEMES[0].terminal.clone(),
    );
    assert_eq!(
        serde_json::to_value(&fixed).expect("the catalog theme serializes"),
        oracle["catalog"]["createdFromPalette"]
    );
    let scheme: crate::theme::derivation::TerminalScheme =
        serde_json::from_value(serde_json::json!({
            "name": "Dracula",
            "background": "#282a36",
            "foreground": "#f8f8f2",
            "palette": [
                "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6",
                "#8be9fd", "#f8f8f2", "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5",
                "#d6acff", "#ff92df", "#a4ffff", "#ffffff"
            ],
            "cursor": "#f8f8f2",
            "cursorText": "#282a36",
            "selectionBackground": "#44475a",
            "selectionForeground": "#ffffff"
        }))
        .expect("a scheme");
    assert_eq!(
        serde_json::to_value(catalog_theme_from_scheme(&scheme))
            .expect("the catalog theme serializes"),
        oracle["catalog"]["createdFromScheme"]
    );
}
