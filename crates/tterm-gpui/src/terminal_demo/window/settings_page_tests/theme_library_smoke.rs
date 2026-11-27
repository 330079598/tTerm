//! Smoke acceptance for the live theme library over a running session.
//!
//! The test drives the production Appearance page and bundled library over two
//! real local PTY sessions. It filters the gallery, applies a dark and then a
//! light catalog theme, and checks that both tabs, sessions, bindings and
//! palettes stay alive while the full catalog ids persist.
//!
//! The screenshot half of the acceptance lives in the debug-only headless
//! capture example, because this test context has no headless renderer. The
//! configuration here is the harness's in-memory copy, so no application
//! config, database, credential, SSH connection or desktop appearance is
//! touched.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::theme_setting::library::{
    LIBRARY_APPLY_ID, LIBRARY_ID, LIBRARY_OPEN_ID, LIBRARY_SEARCH_ID, ThemeFilter, row_id,
};

const DARK_THEME: &str = "Dracula";
const LIGHT_THEME: &str = "Catppuccin Latte";
const DARK_QUERY: &str = "dracula";
const LIGHT_QUERY: &str = "catppuccin latte";
const DARK_MARKER: &str = "THEME-DARK-PTY-OK";
const LIGHT_MARKER: &str = "THEME-LIGHT-PTY-OK";

struct CatalogChoice {
    id: &'static str,
    dark: bool,
}

struct SessionProof {
    first_view: gpui_kit::EntityId,
    second_view: gpui_kit::EntityId,
    first_binding: Rc<TabBinding>,
    second_binding: Rc<TabBinding>,
    first_model: Arc<Terminal>,
    second_model: Arc<Terminal>,
    first_nonce: u32,
    second_nonce: u32,
}

#[gpui_kit::test]
fn filtered_gallery_and_dark_then_light_keep_the_running_session(cx: &mut TestAppContext) {
    let dark = catalog_choice(DARK_THEME);
    let light = catalog_choice(LIGHT_THEME);
    assert!(dark.dark, "{DARK_THEME} should be a dark library theme");
    assert!(!light.dark, "{LIGHT_THEME} should be a light library theme");

    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    let first_model = Arc::clone(&harness.first_model);
    let proof = session_proof(&mut harness, &root);

    send_marker(harness.visual, &first_model, DARK_MARKER);
    open_appearance(&mut harness);
    open_library(&mut harness, &root);
    select_library(&mut harness, &root, &dark, DARK_QUERY, ThemeFilter::Dark);
    apply_library(&mut harness, &root, &config, &dark);
    assert_workspace_theme(&mut harness, &root, &config, &dark);
    close_settings(&mut harness, &root);
    send_marker(harness.visual, &first_model, DARK_MARKER);
    assert_session_proof(&mut harness, &root, &proof);
    assert_output_contains(&first_model, DARK_MARKER);

    open_appearance(&mut harness);
    open_library(&mut harness, &root);
    select_library(&mut harness, &root, &light, LIGHT_QUERY, ThemeFilter::Light);
    apply_library(&mut harness, &root, &config, &light);
    assert_workspace_theme(&mut harness, &root, &config, &light);
    close_settings(&mut harness, &root);
    send_marker(harness.visual, &first_model, LIGHT_MARKER);
    assert_session_proof(&mut harness, &root, &proof);
    assert_output_contains(&first_model, LIGHT_MARKER);

    open_appearance(&mut harness);
    let reopened = harness
        .visual
        .update(|_, cx| root.read(cx).ui.settings_page.read(cx).theme_settings(cx));
    assert_eq!(
        reopened.theme, light.id,
        "reopening the page reloads the persisted light id"
    );
    open_library(&mut harness, &root);
    let selected = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_selected(cx)
    });
    assert_eq!(
        selected.as_deref(),
        Some(light.id),
        "the reopened library highlights the persisted light id"
    );
    close_library_and_settings(&mut harness, &root);

    let persisted = harness
        .visual
        .update(|_, cx| config.read(cx).config().theme.clone());
    let evidence = serde_json::json!({
        "version": 1,
        "method": "GPUI production Prototype and Appearance theme library over two real local PTY sessions",
        "isolation": "in-memory AppConfig and isolated writer; no application config, database, keychain, SSH credential or desktop appearance is read",
        "catalog": {
            "dark": { "name": DARK_THEME, "id": dark.id, "query": DARK_QUERY, "filter": "dark" },
            "light": { "name": LIGHT_THEME, "id": light.id, "query": LIGHT_QUERY, "filter": "light" },
        },
        "sessions": {
            "tabs": [1, 2],
            "active": 1,
            "firstView": format!("{:?}", proof.first_view),
            "secondView": format!("{:?}", proof.second_view),
            "firstNonce": proof.first_nonce,
            "secondNonce": proof.second_nonce,
        },
        "ptyMarkers": [DARK_MARKER, LIGHT_MARKER],
        "persisted": {
            "theme": persisted,
            "reopenedTheme": reopened.theme,
            "reopenedLibrarySelection": selected,
        },
    });
    let directory = evidence_directory();
    fs::create_dir_all(&directory).expect("the evidence directory is writable");
    let path = directory.join("evidence.json");
    fs::write(
        &path,
        serde_json::to_string_pretty(&evidence).expect("the evidence serializes") + "\n",
    )
    .expect("the evidence is written");
    println!("theme library smoke evidence: {}", path.display());

    harness.visual.update(|window, _| window.remove_window());
}

fn catalog_choice(name: &str) -> CatalogChoice {
    let catalog =
        tterm_core::theme::catalog::load_theme_catalog().expect("the bundled theme catalog");
    let theme = catalog
        .iter()
        .find(|theme| theme.name == name)
        .unwrap_or_else(|| panic!("{name} should be in the bundled theme catalog"));
    CatalogChoice {
        id: theme.id.as_str(),
        dark: theme.is_dark(),
    }
}

fn evidence_directory() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.join("target/parity/p4-14-live-theme-library")
        .join(format!("smoke-{}", next_number()))
}

fn next_number() -> u32 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    (millis % 3_000_000_000) as u32 + 1
}

fn session_proof(harness: &mut Harness<'_>, root: &Entity<Prototype>) -> SessionProof {
    let (first_view, second_view) = harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        (
            workspace
                .tabs
                .get(1)
                .expect("the first tab exists")
                .view
                .entity_id(),
            workspace
                .tabs
                .get(2)
                .expect("the second tab exists")
                .view
                .entity_id(),
        )
    });
    SessionProof {
        first_view,
        second_view,
        first_binding: Rc::clone(&harness.first),
        second_binding: Rc::clone(&harness.second),
        first_model: Arc::clone(&harness.first_model),
        second_model: Arc::clone(&harness.second_model),
        first_nonce: harness
            .first
            .session()
            .expect("the first session is live")
            .session_nonce(),
        second_nonce: harness
            .second
            .session()
            .expect("the second session is live")
            .session_nonce(),
    }
}

fn assert_session_proof(harness: &mut Harness<'_>, root: &Entity<Prototype>, proof: &SessionProof) {
    harness.visual.update(|_, cx| {
        let workspace = root.read(cx);
        assert_eq!(workspace.tabs.len(), 2, "both tabs should still exist");
        assert_eq!(
            workspace
                .tabs
                .ordered()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "the tab order should not change"
        );
        assert_eq!(workspace.tabs.active_id(), 1, "the active tab should stay");
        let first = workspace.tabs.get(1).expect("the first tab exists");
        let second = workspace.tabs.get(2).expect("the second tab exists");
        assert_eq!(first.view.entity_id(), proof.first_view);
        assert_eq!(second.view.entity_id(), proof.second_view);
        assert!(Rc::ptr_eq(
            &proof.first_binding,
            first.binding.as_ref().expect("the first binding")
        ));
        assert!(Rc::ptr_eq(
            &proof.second_binding,
            second.binding.as_ref().expect("the second binding")
        ));
        let first_session = proof
            .first_binding
            .session()
            .expect("the first session is live");
        let second_session = proof
            .second_binding
            .session()
            .expect("the second session is live");
        assert!(Arc::ptr_eq(&proof.first_model, &first_session.model()));
        assert!(Arc::ptr_eq(&proof.second_model, &second_session.model()));
        assert_eq!(first_session.session_nonce(), proof.first_nonce);
        assert_eq!(second_session.session_nonce(), proof.second_nonce);
    });
}

fn open_appearance(harness: &mut Harness<'_>) {
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    wait_until("the settings page", || {
        harness.visual.debug_bounds(SETTINGS_PAGE_ID).is_some()
    });
    click(harness.visual, settings_page::SETTINGS_TAB_APPEARANCE_ID);
    wait_until("the appearance section", || {
        harness
            .visual
            .debug_bounds(crate::theme_setting::SECTION_ID)
            .is_some()
    });
}

fn open_library(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    click(harness.visual, LIBRARY_OPEN_ID);
    wait_until("the theme library", || {
        harness.visual.debug_bounds(LIBRARY_ID).is_some()
            && harness.visual.update(|_, cx| {
                root.read(cx)
                    .ui
                    .settings_page
                    .read(cx)
                    .theme_library_open(cx)
            })
    });
}

fn select_library(
    harness: &mut Harness<'_>,
    root: &Entity<Prototype>,
    choice: &CatalogChoice,
    query: &str,
    filter: ThemeFilter,
) {
    click(harness.visual, LIBRARY_SEARCH_ID);
    type_search(harness, query);
    click(harness.visual, filter.selector());
    wait_until("the filtered theme library", || {
        harness.visual.update(|_, cx| {
            let page = root.read(cx).ui.settings_page.read(cx);
            page.theme_library_query(cx) == query
                && page.theme_library_filter(cx) == filter
                && page.theme_library_ids(cx).iter().any(|id| id == choice.id)
        })
    });
    let ids = harness.visual.update(|_, cx| {
        root.read(cx)
            .ui
            .settings_page
            .read(cx)
            .theme_library_ids(cx)
    });
    let catalog =
        tterm_core::theme::catalog::load_theme_catalog().expect("the bundled theme catalog");
    assert!(
        ids.iter().all(|id| catalog
            .iter()
            .find(|theme| &theme.id == id)
            .is_some_and(|theme| theme.is_dark() == (filter == ThemeFilter::Dark))),
        "the {filter:?} filter should only list themes of that tone"
    );
    click_id(harness.visual, &row_id(choice.id));
    wait_until("the theme preview", || {
        harness.visual.update(|_, cx| {
            let page = root.read(cx).ui.settings_page.read(cx);
            page.theme_library_selected(cx).as_deref() == Some(choice.id)
                && root.read(cx).ui.theme.id == choice.id
        })
    });
}

fn apply_library(
    harness: &mut Harness<'_>,
    root: &Entity<Prototype>,
    config: &Entity<ConfigState>,
    choice: &CatalogChoice,
) {
    click(harness.visual, LIBRARY_APPLY_ID);
    wait_until("the theme save", || {
        harness.visual.run_until_parked();
        harness.visual.update(|_, cx| {
            let page = root.read(cx).ui.settings_page.read(cx);
            !page.theme_saving(cx)
                && !config.read(cx).is_save_pending()
                && !page.theme_library_open(cx)
                && page.theme_settings(cx).theme == choice.id
        })
    });
    let saved = harness
        .visual
        .update(|_, cx| config.read(cx).config().theme.clone());
    assert_eq!(
        saved, choice.id,
        "the accepted library id should reach the configuration"
    );
}

fn assert_workspace_theme(
    harness: &mut Harness<'_>,
    root: &Entity<Prototype>,
    config: &Entity<ConfigState>,
    choice: &CatalogChoice,
) {
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let resolved =
            crate::app_window::theme::resolve(config.read(cx).config(), workspace.system_dark);
        assert_eq!(resolved.id, choice.id);
        assert_eq!(workspace.ui.theme.id, choice.id);
        assert_eq!(workspace.ui.theme, resolved.ui);
        for id in [1, 2] {
            let tab = workspace.tabs.get(id).expect("the tab exists");
            let palette = resolved.palette_for_tab(tab.saved());
            assert_eq!(tab.view.read(cx).palette(), &palette);
            assert_eq!(
                tab.binding
                    .as_ref()
                    .expect("the tab binding")
                    .session()
                    .expect("the session is live")
                    .palette(),
                palette
            );
        }
    });
}

fn close_settings(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    wait_until("the settings route to close", || {
        harness
            .visual
            .update(|_, cx| !root.read(cx).settings_is_open())
    });
}

fn close_library_and_settings(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    click(
        harness.visual,
        crate::theme_setting::library::LIBRARY_CANCEL_ID,
    );
    wait_until("the theme library to close", || {
        harness.visual.update(|_, cx| {
            !root
                .read(cx)
                .ui
                .settings_page
                .read(cx)
                .theme_library_open(cx)
        })
    });
    close_settings(harness, root);
}

fn assert_output_contains(model: &Terminal, marker: &str) {
    assert!(
        output(model).contains(marker),
        "the running PTY should still echo {marker}"
    );
}
