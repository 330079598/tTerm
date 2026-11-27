//! Pure rules for the theme cards and the follow-system slots.

use super::*;

#[test]
fn all_six_presets_have_a_card_and_only_light_is_a_light_theme() {
    assert_eq!(PRESETS.len(), 6);
    assert!(!is_dark("light"));
    for id in ["default", "ocean", "forest", "sunset", "ubuntu"] {
        assert!(is_dark(id), "{id} is a dark preset");
    }
}

#[test]
fn every_slot_has_a_stable_selector_per_preset() {
    assert_eq!(
        option_id(ThemeSlot::Single, "ocean"),
        "settings-theme-option-single-ocean"
    );
    assert_eq!(
        option_id(ThemeSlot::Light, "light"),
        "settings-theme-option-light-light"
    );
    assert_eq!(
        option_id(ThemeSlot::Dark, "forest"),
        "settings-theme-option-dark-forest"
    );
}

#[test]
fn the_builtin_ids_are_the_names_the_config_holds() {
    assert_eq!(
        PRESETS,
        ["default", "light", "ocean", "forest", "sunset", "ubuntu"]
    );
}

#[test]
fn the_library_rows_have_a_stable_selector() {
    assert_eq!(
        super::library::row_id("catalog:Dracula"),
        "settings-theme-library-row-catalog:Dracula"
    );
    assert_eq!(
        super::library::ThemeFilter::Dark.selector(),
        "settings-theme-library-dark"
    );
}

#[test]
fn every_library_entry_is_in_exactly_one_tone_filter() {
    use super::library::{ThemeFilter, visible_themes};
    use tterm_core::theme::catalog::load_theme_catalog;

    let catalog = load_theme_catalog().expect("the bundled theme library");
    let all = visible_themes(catalog, "", ThemeFilter::All);
    let dark = visible_themes(catalog, "", ThemeFilter::Dark);
    let light = visible_themes(catalog, "", ThemeFilter::Light);

    assert_eq!(all.len(), catalog.len());
    assert!(!dark.is_empty(), "the library has dark themes");
    assert!(!light.is_empty(), "the library has light themes");
    assert_eq!(dark.len() + light.len(), all.len());
    assert!(dark.iter().all(|theme| theme.is_dark()));
    assert!(light.iter().all(|theme| !theme.is_dark()));
}

#[test]
fn search_matches_every_word_of_a_name_case_insensitively() {
    use super::library::{ThemeFilter, visible_themes};
    use tterm_core::theme::catalog::load_theme_catalog;

    let catalog = load_theme_catalog().expect("the bundled theme library");
    let sample = catalog.first().expect("a bundled theme");
    let word = sample.name.to_lowercase();
    let matches = visible_themes(catalog, &word.to_uppercase(), ThemeFilter::All);
    assert!(
        matches.iter().any(|theme| theme.id == sample.id),
        "the name search finds its own theme"
    );
    assert!(
        visible_themes(catalog, "no-such-theme-anywhere", ThemeFilter::All).is_empty(),
        "an unmatched name leaves an empty list"
    );
}

#[test]
fn library_darkness_uses_the_shared_palette_rule() {
    use tterm_core::theme::catalog::load_theme_catalog;

    let catalog = load_theme_catalog().expect("the bundled theme library");
    let dark = catalog
        .iter()
        .find(|theme| theme.is_dark())
        .expect("a dark library theme");
    let light = catalog
        .iter()
        .find(|theme| !theme.is_dark())
        .expect("a light library theme");

    assert!(theme_is_dark(&dark.id));
    assert!(!theme_is_dark(&light.id));
    assert!(theme_is_dark("ocean"));
    assert!(!theme_is_dark("light"));
    assert!(!theme_is_dark("custom-from-tauri"));
}
