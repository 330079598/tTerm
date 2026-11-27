//! Theme resolution tests against the saved config, the shared presets and
//! the bundled library.

use tterm_core::config::AppConfig;
use tterm_core::theme::catalog::load_theme_catalog;
use tterm_core::theme::definitions::get_preset_theme;
use tterm_core::theme::types::CatalogTheme;
use tterm_core::workspace::tab::Tab;
use tterm_terminal::palette::Rgba as PaletteColor;
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::palette::{DARK, FOREST, LIGHT, OCEAN, SUNSET, UBUNTU};
use tterm_ui::tokens::theme::Theme;

use super::{DEFAULT_THEME_ID, ResolvedTheme, resolve};

fn config(theme: &str) -> AppConfig {
    AppConfig {
        theme: theme.to_string(),
        ..AppConfig::default()
    }
}

fn catalog() -> &'static [CatalogTheme] {
    load_theme_catalog().expect("the bundled catalog")
}

fn light_theme() -> &'static CatalogTheme {
    catalog()
        .iter()
        .find(|theme| !theme.is_dark())
        .expect("the library has a light theme")
}

fn dark_theme() -> &'static CatalogTheme {
    catalog()
        .iter()
        .find(|theme| theme.is_dark())
        .expect("the library has a dark theme")
}

fn assert_builtin(theme_id: &str, expected_id: &str, expected_ui: Theme) {
    let resolved = resolve(&config(theme_id), true);
    assert_eq!(resolved.id, expected_id, "resolved id for {theme_id}");
    assert_eq!(resolved.ui, expected_ui, "interface palette for {theme_id}");
    let preset = get_preset_theme(expected_id).expect("the built-in theme exists");
    assert_eq!(
        resolved.terminal, preset.terminal,
        "ANSI palette for {theme_id}"
    );
}

#[test]
fn stock_dark_light_and_every_builtin_id_use_the_shared_palettes() {
    for (theme_id, expected_id, expected_ui) in [
        ("default", "default", DARK),
        ("dark", "default", DARK),
        ("light", "light", LIGHT),
        ("ocean", "ocean", OCEAN),
        ("forest", "forest", FOREST),
        ("sunset", "sunset", SUNSET),
        ("ubuntu", "ubuntu", UBUNTU),
    ] {
        assert_builtin(theme_id, expected_id, expected_ui);
    }
}

#[test]
fn a_derived_library_token_keeps_its_hsl_components() {
    assert_eq!(super::ui_hsl("220 13% 12%"), Hsl::new(220.0, 13.0, 12.0));
    assert_eq!(
        super::ui_hsl("not a colour"),
        Hsl::new(0.0, 0.0, 0.0),
        "an impossible token falls back instead of panicking"
    );
}

#[test]
fn every_library_theme_resolves_with_its_own_id_colors_and_darkness() {
    let themes = catalog();
    assert!(themes.len() > 600, "the library is the bundled one");
    for theme in themes {
        let resolved = resolve(&config(&theme.id), true);
        assert_eq!(resolved.id, theme.id, "{}", theme.name);
        assert_eq!(resolved.ui.id, theme.id, "{}", theme.name);
        assert_eq!(
            resolved.ui.uses_dark_variant(),
            theme.is_dark(),
            "{}",
            theme.name
        );
        assert_eq!(
            resolved.terminal,
            *theme.terminal(),
            "ANSI palette for {}",
            theme.name
        );
        assert_eq!(
            resolved.ui.colors,
            super::ui_colors(theme.colors()),
            "interface palette for {}",
            theme.name
        );
    }
}

#[test]
fn light_and_dark_library_themes_keep_exact_ui_and_ansi_palettes() {
    for theme in [light_theme(), dark_theme()] {
        let resolved = resolve(&config(&theme.id), true);
        let colors = theme.colors();

        assert_eq!(resolved.ui.colors, super::ui_colors(colors));
        assert_eq!(
            resolved.ui.colors.background,
            super::ui_hsl(&colors.background)
        );
        assert_eq!(
            resolved.ui.colors.foreground,
            super::ui_hsl(&colors.foreground)
        );
        assert_eq!(resolved.ui.colors.primary, super::ui_hsl(&colors.primary));
        assert_eq!(resolved.terminal, *theme.terminal(), "{}", theme.name);
        assert_eq!(
            resolved.terminal_palette(),
            theme.terminal().to_terminal_palette(),
            "{}",
            theme.name
        );
        assert_eq!(resolved.terminal.blue, theme.terminal().blue);
    }

    assert!(
        !resolve(&config(&light_theme().id), true)
            .ui
            .uses_dark_variant(),
        "a library theme is light without being named light"
    );
    assert!(
        resolve(&config(&dark_theme().id), true)
            .ui
            .uses_dark_variant(),
        "a library theme is dark without being named dark"
    );
}

#[test]
fn following_the_system_selects_the_saved_slot_theme() {
    let following = AppConfig {
        theme: "ubuntu".to_string(),
        theme_follow_system: true,
        theme_light: "ocean".to_string(),
        theme_dark: "sunset".to_string(),
        ..AppConfig::default()
    };

    let light = resolve(&following, false);
    assert_eq!(light.id, "ocean");
    assert_eq!(light.ui, OCEAN);

    let dark = resolve(&following, true);
    assert_eq!(dark.id, "sunset");
    assert_eq!(dark.ui, SUNSET);

    let explicit = resolve(
        &AppConfig {
            theme_follow_system: false,
            ..following
        },
        false,
    );
    assert_eq!(explicit.id, "ubuntu");
    assert_eq!(explicit.ui, UBUNTU);
}

#[test]
fn following_the_system_selects_library_themes_in_each_slot() {
    let light = light_theme();
    let dark = dark_theme();
    let following = AppConfig {
        theme: "ubuntu".to_string(),
        theme_follow_system: true,
        theme_light: light.id.clone(),
        theme_dark: dark.id.clone(),
        ..AppConfig::default()
    };

    let resolved_light = resolve(&following, false);
    assert_eq!(resolved_light.id, light.id);
    assert_eq!(resolved_light.ui.id, light.id);
    assert!(!resolved_light.ui.uses_dark_variant());
    assert_eq!(resolved_light.terminal, *light.terminal());

    let resolved_dark = resolve(&following, true);
    assert_eq!(resolved_dark.id, dark.id);
    assert_eq!(resolved_dark.ui.id, dark.id);
    assert!(resolved_dark.ui.uses_dark_variant());
    assert_eq!(resolved_dark.terminal, *dark.terminal());
}

#[test]
fn unknown_empty_and_non_builtin_ids_fall_back_to_the_default_theme() {
    for theme_id in [
        "",
        "missing",
        "dark-theme",
        "Solarized Dark",
        "catalog:",
        "catalog:No Such Bundled Theme",
        "catalog:nord",
        "default ",
    ] {
        let config = config(theme_id);
        let resolved = resolve(&config, true);
        assert_eq!(
            resolved.id, DEFAULT_THEME_ID,
            "{theme_id:?} resolves to the default"
        );
        assert_eq!(resolved.ui, DARK, "{theme_id:?} uses the default palette");
        let default = get_preset_theme(DEFAULT_THEME_ID).expect("the default theme exists");
        assert_eq!(
            resolved.terminal, default.terminal,
            "{theme_id:?} uses the default ANSI palette"
        );
        assert_eq!(
            config.theme, theme_id,
            "resolution does not rewrite the saved id"
        );
    }
}

#[test]
fn an_invalid_followed_slot_falls_back_without_changing_the_other_slot() {
    let following = AppConfig {
        theme_follow_system: true,
        theme_light: "not-a-theme".to_string(),
        theme_dark: "forest".to_string(),
        ..AppConfig::default()
    };

    assert_eq!(resolve(&following, false).id, DEFAULT_THEME_ID);
    assert_eq!(resolve(&following, true).id, "forest");
}

#[test]
fn interface_and_ansi_palettes_come_from_their_own_sources() {
    let resolved = resolve(&config("ocean"), true);

    assert_eq!(resolved.ui.colors.background, OCEAN.colors.background);
    assert_eq!(resolved.terminal.background, "hsl(200 30% 10%)");
    assert_eq!(
        resolved.terminal_palette().blue,
        PaletteColor::rgb(0x4d, 0xb5, 0xff),
        "the ANSI blue is the one the terminal theme declares"
    );
    assert_ne!(
        resolved.terminal_palette().blue,
        resolved.terminal_palette().background,
        "the ANSI palette is not the interface surface"
    );
}

#[test]
fn a_connection_theme_overrides_the_application_theme() {
    let app = resolve(&config("light"), false);
    let app_palette = app.terminal_palette();

    assert_eq!(app.terminal_palette_for(None), app_palette);
    assert_eq!(
        app.terminal_palette_for(Some("ocean")),
        get_preset_theme("ocean")
            .expect("the ocean theme exists")
            .terminal
            .to_terminal_palette()
    );
    assert_eq!(
        app.terminal_palette_for(Some("dark")),
        get_preset_theme(DEFAULT_THEME_ID)
            .expect("the default theme exists")
            .terminal
            .to_terminal_palette()
    );
    assert_eq!(
        app.terminal_palette_for(Some("missing")),
        app_palette,
        "an unknown connection theme falls back to the application theme"
    );
}

#[test]
fn a_connection_theme_can_name_a_library_theme() {
    let theme = dark_theme();
    let expected = theme.terminal().to_terminal_palette();
    let app = resolve(&config("light"), false);
    let tab: Tab = serde_json::from_value(serde_json::json!({
        "id": "ssh-tab",
        "title": "SSH",
        "type": "ssh",
        "connection": { "terminalTheme": theme.id },
    }))
    .expect("the tab deserializes");

    assert_eq!(app.terminal_palette_for(Some(theme.id.as_str())), expected);
    assert_eq!(
        app.palette_for_tab(&tab),
        expected,
        "the SSH connection's library theme reaches its tab"
    );
    assert_eq!(
        app.terminal_palette_for(Some("catalog:No Such Bundled Theme")),
        app.terminal_palette(),
        "an unknown library theme falls back to the application theme"
    );
}

#[test]
fn the_default_resolution_paints_the_dark_workspace_background() {
    let resolved = ResolvedTheme::default();
    assert_eq!(resolved.id, "default");
    assert_eq!(resolved.ui, DARK);
    assert_eq!(
        resolved.background(),
        hsla(DARK.colors.background, 1.0).to_rgb()
    );
    assert_eq!(
        resolved.terminal,
        get_preset_theme("default")
            .expect("default theme")
            .terminal
            .clone()
    );
}
