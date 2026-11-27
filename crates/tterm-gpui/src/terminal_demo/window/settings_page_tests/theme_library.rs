//! Functional route tests for the theme library on the Appearance page.

use super::*;
use crate::theme_setting::library::{
    LIBRARY_APPLY_ID, LIBRARY_CANCEL_ID, LIBRARY_DARK_ID, LIBRARY_LIGHT_ID, LIBRARY_OPEN_ID,
    LIBRARY_SEARCH_ID, ThemeFilter, row_id,
};
use tterm_core::theme::catalog::load_theme_catalog;
use tterm_core::theme::types::CatalogTheme;

fn catalog_theme(dark: bool) -> &'static CatalogTheme {
    load_theme_catalog()
        .expect("the bundled theme library")
        .iter()
        .find(|theme| theme.is_dark() == dark)
        .expect("the requested tone exists")
}

fn open_appearance(harness: &mut Harness<'_>) {
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, settings_page::SETTINGS_TAB_APPEARANCE_ID);
}

fn open_library(harness: &mut Harness<'_>) {
    click(harness.visual, LIBRARY_OPEN_ID);
}

fn wait_for(harness: &mut Harness<'_>, what: &str, mut condition: impl FnMut(&App) -> bool) {
    wait_until(what, || {
        harness.visual.run_until_parked();
        harness.visual.update(|window, cx| {
            window.render_frame(cx);
            condition(cx)
        })
    });
}

fn library_open(root: &Entity<Prototype>, cx: &App) -> bool {
    root.read(cx)
        .ui
        .settings_page
        .read(cx)
        .theme_library_open(cx)
}

#[gpui_kit::test]
fn the_library_searches_filters_previews_and_cancels_without_saving(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let synced = harness.config.clone();
    let theme = catalog_theme(true);
    let query = theme.name.to_lowercase();

    open_appearance(&mut harness);
    open_library(&mut harness);
    wait_for(&mut harness, "the theme library to open", |cx| {
        library_open(&root, cx)
    });

    click(harness.visual, LIBRARY_SEARCH_ID);
    type_search(&mut harness, &query);
    click(harness.visual, LIBRARY_DARK_ID);
    wait_for(&mut harness, "the filtered library", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        page.theme_library_query(cx) == query
            && page.theme_library_filter(cx) == ThemeFilter::Dark
            && page.theme_library_ids(cx).iter().any(|id| id == &theme.id)
    });

    click_id(harness.visual, &row_id(&theme.id));
    wait_for(&mut harness, "the transient preview", |cx| {
        library_open(&root, cx) && root.read(cx).ui.theme.id == theme.id
    });
    harness.visual.update(|_, cx| {
        assert_eq!(
            synced.read(cx).config().theme,
            "default",
            "browsing never writes the saved theme"
        );
        assert!(!synced.read(cx).is_save_pending());
    });

    press(harness.visual, "escape");
    wait_for(&mut harness, "the preview to be restored", |cx| {
        !library_open(&root, cx) && root.read(cx).factory.theme.id == "default"
    });
    harness.visual.update(|_, cx| {
        assert_eq!(synced.read(cx).config().theme, "default");
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn the_library_applies_a_light_theme_and_keeps_the_other_follow_slot(cx: &mut TestAppContext) {
    let config = AppConfig {
        theme_follow_system: true,
        theme_light: "light".to_string(),
        theme_dark: "default".to_string(),
        ..AppConfig::default()
    };
    let mut harness = harness(cx, config);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    let light = catalog_theme(false);
    let dark = catalog_theme(true);
    let light_query = light.name.to_lowercase();
    let dark_query = dark.name.to_lowercase();

    open_appearance(&mut harness);
    open_library(&mut harness);
    click(harness.visual, LIBRARY_SEARCH_ID);
    type_search(&mut harness, &light_query);
    click(harness.visual, LIBRARY_LIGHT_ID);
    wait_for(&mut harness, "the light library theme", |cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_ids(cx)
            .iter()
            .any(|id| id == &light.id)
    });
    click_id(harness.visual, &row_id(&light.id));
    click(harness.visual, LIBRARY_APPLY_ID);
    wait_for(&mut harness, "the light theme to save", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        !page.theme_saving(cx)
            && !page.theme_library_open(cx)
            && synced.read(cx).config().theme_light == light.id
    });
    harness.visual.update(|_, cx| {
        let saved = synced.read(cx).config();
        assert!(saved.theme_follow_system, "Following stays on");
        assert_eq!(saved.theme_light, light.id, "the light slot changed");
        assert_eq!(saved.theme_dark, "default", "the dark slot is untouched");
        assert_eq!(saved.theme, "default", "the single slot is untouched");
        assert_eq!(root.read(cx).ui.theme.id, light.id);
    });

    open_library(&mut harness);
    click(harness.visual, LIBRARY_SEARCH_ID);
    type_search(&mut harness, &dark_query);
    click(harness.visual, LIBRARY_DARK_ID);
    wait_for(&mut harness, "the dark library theme", |cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_ids(cx)
            .iter()
            .any(|id| id == &dark.id)
    });
    click_id(harness.visual, &row_id(&dark.id));
    click(harness.visual, LIBRARY_APPLY_ID);
    wait_for(&mut harness, "the dark slot to save", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        !page.theme_saving(cx)
            && !page.theme_library_open(cx)
            && synced.read(cx).config().theme_dark == dark.id
    });
    harness.visual.update(|_, cx| {
        let saved = synced.read(cx).config();
        assert_eq!(saved.theme_light, light.id, "the light slot is untouched");
        assert_eq!(saved.theme_dark, dark.id, "the dark slot changed");
        assert_eq!(saved.theme, "default", "the single slot is untouched");
        assert_eq!(
            root.read(cx).ui.theme.id,
            light.id,
            "the light appearance still shows the light slot"
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn the_library_browses_with_arrows_and_enters_the_highlighted_theme(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let synced = harness.config.clone();
    open_appearance(&mut harness);
    open_library(&mut harness);
    click(harness.visual, LIBRARY_SEARCH_ID);
    wait_for(&mut harness, "the unfiltered library", |cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_ids(cx)
            .len()
            > 600
    });
    let ids = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_ids(cx)
    });

    press(harness.visual, "down");
    wait_for(&mut harness, "the first highlighted theme", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        page.theme_library_selected(cx).as_deref() == Some(ids[0].as_str())
            && root.read(cx).ui.theme.id == ids[0]
    });
    press(harness.visual, "down");
    wait_for(&mut harness, "the second highlighted theme", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        page.theme_library_selected(cx).as_deref() == Some(ids[1].as_str())
            && root.read(cx).ui.theme.id == ids[1]
    });

    press(harness.visual, "enter");
    wait_for(&mut harness, "the highlighted theme to save", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        !page.theme_saving(cx)
            && !page.theme_library_open(cx)
            && synced.read(cx).config().theme == ids[1]
    });
    harness.visual.update(|_, cx| {
        assert_eq!(root.read(cx).ui.theme.id, ids[1]);
        assert_eq!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .theme_settings(cx)
                .theme,
            ids[1]
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_library_save_keeps_the_preview_and_reports_the_error(cx: &mut TestAppContext) {
    let writer: ThemeSettingsWriter =
        Arc::new(|_, _| Err("the config file is read-only".to_string()));
    let mut harness = harness_with_theme_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    let theme = catalog_theme(true);
    open_appearance(&mut harness);
    open_library(&mut harness);
    click_id(harness.visual, &row_id(&theme.id));
    wait_for(&mut harness, "the preview", |cx| {
        root.read(cx).ui.theme.id == theme.id
    });
    click(harness.visual, LIBRARY_APPLY_ID);
    wait_for(&mut harness, "the failed save", |cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        !page.theme_saving(cx) && page.theme_error(cx).is_some()
    });
    harness.visual.update(|_, cx| {
        let page = root.read(cx).ui.settings_page.read(cx);
        assert!(
            page.theme_library_open(cx),
            "a failed save keeps the browser open"
        );
        assert_eq!(
            page.theme_error(cx).as_deref(),
            Some("the config file is read-only")
        );
        assert_eq!(synced.read(cx).config().theme, "default");
        assert_eq!(root.read(cx).ui.theme.id, theme.id);
    });

    click(harness.visual, LIBRARY_CANCEL_ID);
    wait_for(&mut harness, "cancel to restore the saved theme", |cx| {
        !library_open(&root, cx) && root.read(cx).factory.theme.id == "default"
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn closing_the_settings_route_restores_a_library_preview(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let theme = catalog_theme(true);
    open_appearance(&mut harness);
    open_library(&mut harness);
    click_id(harness.visual, &row_id(&theme.id));
    wait_for(&mut harness, "the preview", |cx| {
        root.read(cx).ui.theme.id == theme.id
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    wait_for(&mut harness, "the route close to restore the theme", |cx| {
        !root.read(cx).settings_is_open() && root.read(cx).factory.theme.id == "default"
    });
    harness.visual.update(|_, cx| {
        assert_eq!(
            root.read(cx).ui.theme,
            crate::app_window::theme::default_theme().ui
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}
