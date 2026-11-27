//! Route tests for the Appearance page's built-in theme choice.

use super::*;
use crate::theme_setting::{
    DARK_GROUP_ID, FOLLOW_SWITCH_ID, LIGHT_GROUP_ID, PRESETS, ThemeSlot, option_id,
};

fn bounds(
    visual: &mut VisualTestContext,
    id: String,
) -> Option<gpui_kit::Bounds<gpui_kit::Pixels>> {
    // The debug-selector lookup requires a `'static` id; leaking one test
    // string per assertion keeps generated card ids usable.
    let id: &'static str = Box::leak(id.into_boxed_str());
    visual.debug_bounds(id)
}

fn open_appearance(harness: &mut Harness<'_>) {
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, settings_page::SETTINGS_TAB_APPEARANCE_ID);
}

fn wait_for_theme_idle(harness: &mut Harness<'_>, root: &Entity<Prototype>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let idle = harness.visual.update(|_, cx| {
            let page = root.read(cx).ui.settings_page.read(cx);
            !page.theme_saving(cx) && !harness.config.read(cx).is_save_pending()
        });
        if idle {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the theme save"
        );
        harness.visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[gpui_kit::test]
fn the_appearance_tab_offers_six_presets_and_applies_a_choice_live(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    open_appearance(&mut harness);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.active_tab(), SettingsTab::Appearance);
        assert_eq!(page.theme_settings(cx).theme, "default");
    });
    for id in PRESETS {
        assert!(
            bounds(harness.visual, option_id(ThemeSlot::Single, id)).is_some(),
            "{id} should have one card"
        );
    }
    click_id(harness.visual, &option_id(ThemeSlot::Single, "ocean"));
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        let settings = page.theme_settings(cx);
        assert_eq!(settings.theme, "ocean");
        assert!(!settings.follow_system);
        assert_eq!(config.read(cx).config().theme, "ocean");
        assert_eq!(
            root.read(cx).ui.theme.id,
            "ocean",
            "the accepted choice repaints this window without a restart"
        );
    });

    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    open_appearance(&mut harness);
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert_eq!(
            root.read(cx)
                .ui
                .settings_page
                .read(cx)
                .theme_settings(cx)
                .theme,
            "ocean",
            "reopening the page reloads the saved choice"
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_live_theme_change_repaints_existing_inactive_and_future_tabs(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    let first = Rc::clone(&harness.first);
    let second = Rc::clone(&harness.second);
    let first_model = Arc::clone(&harness.first_model);
    let second_model = Arc::clone(&harness.second_model);

    let (third, first_view, second_view, third_view) = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, test_shell(), &workspace.ui.locale, cx)
            });
            let third = tab_binding(workspace, 3);
            workspace.select_tab(1, window, cx);
            (
                third,
                workspace.tabs.get(1).expect("first tab").view.entity_id(),
                workspace.tabs.get(2).expect("second tab").view.entity_id(),
                workspace.tabs.get(3).expect("third tab").view.entity_id(),
            )
        })
    });
    let first_nonce = first.session().expect("first session").session_nonce();
    let second_nonce = second.session().expect("second session").session_nonce();
    let third_nonce = third.session().expect("third session").session_nonce();

    open_appearance(&mut harness);
    click_id(harness.visual, &option_id(ThemeSlot::Single, "ocean"));
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let resolved =
            crate::app_window::theme::resolve(config.read(cx).config(), workspace.system_dark);
        assert_eq!(workspace.ui.theme, resolved.ui);
        assert_eq!(workspace.ui.background, resolved.background());
        let page = workspace.ui.settings_page.read(cx);
        assert_eq!(page.theme(), resolved.ui);
        assert_eq!(page.theme_setting_theme(cx), resolved.ui);

        for id in [1, 2, 3] {
            let tab = workspace.tabs.get(id).expect("the tab exists");
            let palette = resolved.palette_for_tab(tab.saved());
            assert_eq!(tab.view.read(cx).palette(), &palette);
            let binding = tab.binding.as_ref().expect("the tab has a live binding");
            assert_eq!(
                binding.session().expect("the session is live").palette(),
                palette
            );
        }

        let first_tab = workspace.tabs.get(1).expect("first tab");
        let second_tab = workspace.tabs.get(2).expect("second tab");
        let third_tab = workspace.tabs.get(3).expect("third tab");
        assert_eq!(first_tab.view.entity_id(), first_view);
        assert_eq!(second_tab.view.entity_id(), second_view);
        assert_eq!(third_tab.view.entity_id(), third_view);
        assert!(Rc::ptr_eq(
            &first,
            first_tab.binding.as_ref().expect("first binding")
        ));
        assert!(Rc::ptr_eq(
            &second,
            second_tab.binding.as_ref().expect("second binding")
        ));
        assert!(Rc::ptr_eq(
            &third,
            third_tab.binding.as_ref().expect("third binding")
        ));
        assert!(Arc::ptr_eq(
            &first_model,
            &first_tab
                .binding
                .as_ref()
                .expect("first binding")
                .session()
                .expect("first session")
                .model()
        ));
        assert!(Arc::ptr_eq(
            &second_model,
            &second_tab
                .binding
                .as_ref()
                .expect("second binding")
                .session()
                .expect("second session")
                .model()
        ));
        assert_eq!(
            first.session().expect("first session").session_nonce(),
            first_nonce
        );
        assert_eq!(
            second.session().expect("second session").session_nonce(),
            second_nonce
        );
        assert_eq!(
            third.session().expect("third session").session_nonce(),
            third_nonce
        );
    });

    let (future, future_view) = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, test_shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            let tab = workspace.tabs.get(4).expect("future tab");
            (tab_binding(workspace, 4), tab.view.entity_id())
        })
    });
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let resolved =
            crate::app_window::theme::resolve(config.read(cx).config(), root.read(cx).system_dark);
        let workspace = root.read(cx);
        let tab = workspace.tabs.get(4).expect("future tab");
        assert_eq!(tab.view.entity_id(), future_view);
        assert_eq!(tab.view.read(cx).palette(), &resolved.terminal_palette());
        assert_eq!(
            future.session().expect("future session").palette(),
            resolved.terminal_palette()
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn following_the_system_stays_on_when_a_dark_preset_is_chosen(cx: &mut TestAppContext) {
    let config = AppConfig {
        theme_follow_system: true,
        theme_light: "light".to_string(),
        theme_dark: "default".to_string(),
        ..AppConfig::default()
    };
    let mut harness = harness(cx, config);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    open_appearance(&mut harness);

    harness.visual.update(|window, cx| window.render_frame(cx));
    assert!(harness.visual.debug_bounds(LIGHT_GROUP_ID).is_some());
    assert!(harness.visual.debug_bounds(DARK_GROUP_ID).is_some());
    assert!(
        bounds(harness.visual, option_id(ThemeSlot::Single, "ocean")).is_none(),
        "following the system shows the two slots instead of one list"
    );

    click_id(harness.visual, &option_id(ThemeSlot::Dark, "ocean"));
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        let settings = page.theme_settings(cx);
        assert!(
            settings.follow_system,
            "choosing a theme keeps Following on"
        );
        assert_eq!(settings.theme_dark, "ocean");
        assert_eq!(settings.theme, "default", "only the chosen slot changed");
        assert!(synced.read(cx).config().theme_follow_system);
        assert_eq!(synced.read(cx).config().theme_dark, "ocean");
    });
    assert!(
        harness.visual.debug_bounds(DARK_GROUP_ID).is_some(),
        "the page still shows the slots after the save"
    );

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn turning_following_off_freezes_the_slot_the_window_shows(cx: &mut TestAppContext) {
    let config = AppConfig {
        theme_follow_system: true,
        theme_light: "forest".to_string(),
        theme_dark: "ocean".to_string(),
        ..AppConfig::default()
    };
    let mut harness = harness(cx, config);
    let root = harness.root.clone();
    open_appearance(&mut harness);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(
            !root
                .read(cx)
                .ui
                .settings_page
                .read(cx)
                .theme_system_dark(cx),
            "the test window opens with a light appearance"
        );
    });
    click(harness.visual, FOLLOW_SWITCH_ID);
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let settings = root.read(cx).ui.settings_page.read(cx).theme_settings(cx);
        assert!(!settings.follow_system, "the user turned Following off");
        assert_eq!(settings.theme, "forest", "the light slot is frozen");
        assert_eq!(settings.theme_light, "forest");
        assert_eq!(settings.theme_dark, "ocean");
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_theme_search_result_opens_the_appearance_page(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);
    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "choose theme");
    press(harness.visual, "enter");

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.active_tab(), SettingsTab::Appearance);
        assert!(
            page.theme_focus(cx).is_focused(window),
            "the result focuses the theme section"
        );
    });

    click(harness.visual, settings_page::SETTINGS_SEARCH_FIELD_ID);
    type_search(&mut harness, "follow system");
    press(harness.visual, "enter");
    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.active_tab(), SettingsTab::Appearance);
        assert!(
            page.theme_follow_focus(cx).is_focused(window),
            "the follow-system result focuses its switch"
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn following_on_keeps_a_custom_theme_id_the_native_client_cannot_show(cx: &mut TestAppContext) {
    let config = AppConfig {
        theme: "custom-from-tauri".to_string(),
        ..AppConfig::default()
    };
    let mut harness = harness(cx, config);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    open_appearance(&mut harness);

    click(harness.visual, FOLLOW_SWITCH_ID);
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let settings = root.read(cx).ui.settings_page.read(cx).theme_settings(cx);
        assert!(settings.follow_system);
        assert_eq!(settings.theme, "custom-from-tauri");
        assert_eq!(synced.read(cx).config().theme, "custom-from-tauri");
        assert_eq!(
            synced.read(cx).config().theme_light,
            AppConfig::default().theme_light
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_failed_theme_save_keeps_the_choice_and_reports_the_error(cx: &mut TestAppContext) {
    let writer: ThemeSettingsWriter =
        Arc::new(|_, _| Err("the config file is read-only".to_string()));
    let mut harness = harness_with_theme_writer(cx, AppConfig::default(), writer);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    open_appearance(&mut harness);

    click_id(harness.visual, &option_id(ThemeSlot::Single, "sunset"));
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let page = root.read(cx).ui.settings_page.read(cx);
        assert_eq!(page.theme_settings(cx).theme, "sunset");
        assert_eq!(
            page.theme_error(cx).as_deref(),
            Some("the config file is read-only")
        );
        assert_eq!(synced.read(cx).config().theme, "default");
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn following_the_system_applies_both_saved_slots_live(cx: &mut TestAppContext) {
    let config = AppConfig {
        theme_follow_system: true,
        theme_light: "forest".to_string(),
        theme_dark: "ocean".to_string(),
        ..AppConfig::default()
    };
    let harness = harness(cx, config);
    let root = harness.root.clone();
    let synced = harness.config.clone();
    let first = Rc::clone(&harness.first);
    let first_model = Arc::clone(&harness.first_model);
    let first_view = harness.visual.update(|_, cx| {
        root.read(cx)
            .tabs
            .get(1)
            .expect("first tab")
            .view
            .entity_id()
    });

    harness.visual.update(|_, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_system_dark_for_test(true, cx);
        });
        let workspace = root.read(cx);
        let resolved =
            crate::app_window::theme::resolve(synced.read(cx).config(), workspace.system_dark);
        assert_eq!(resolved.id, "ocean");
        assert_eq!(workspace.ui.theme.id, "ocean");
        let page = workspace.ui.settings_page.read(cx);
        assert_eq!(page.theme(), resolved.ui);
        assert_eq!(page.theme_setting_theme(cx), resolved.ui);
        let tab = workspace.tabs.get(1).expect("first tab");
        assert_eq!(tab.view.read(cx).palette(), &resolved.terminal_palette());
        assert_eq!(
            first.session().expect("first session").palette(),
            resolved.terminal_palette()
        );
        assert_eq!(tab.view.entity_id(), first_view);
        assert!(Rc::ptr_eq(&first, tab.binding.as_ref().expect("binding")));
        assert!(Arc::ptr_eq(
            &first_model,
            &tab.binding
                .as_ref()
                .expect("binding")
                .session()
                .expect("first session")
                .model()
        ));
    });

    harness.visual.update(|_, cx| {
        root.update(cx, |workspace, cx| {
            workspace.set_system_dark_for_test(false, cx);
        });
        let workspace = root.read(cx);
        let resolved =
            crate::app_window::theme::resolve(synced.read(cx).config(), workspace.system_dark);
        assert_eq!(resolved.id, "forest");
        assert_eq!(workspace.ui.theme.id, "forest");
        let tab = workspace.tabs.get(1).expect("first tab");
        assert_eq!(tab.view.read(cx).palette(), &resolved.terminal_palette());
        assert_eq!(
            first.session().expect("first session").palette(),
            resolved.terminal_palette()
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn a_connection_theme_still_overrides_a_live_application_theme_change(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    let root = harness.root.clone();
    let config = harness.config.clone();
    let first = Rc::clone(&harness.first);

    harness.visual.update(|_, cx| {
        root.update(cx, |workspace, _| {
            let mut connection = tterm_core::workspace::tab::TabConnection::default();
            connection.set_terminal_theme(Some("forest".to_string()));
            workspace
                .tabs
                .get_mut(1)
                .expect("first tab")
                .saved
                .set_connection(Some(connection));
        });
    });
    open_appearance(&mut harness);
    click_id(harness.visual, &option_id(ThemeSlot::Single, "ocean"));
    wait_for_theme_idle(&mut harness, &root);

    harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let resolved =
            crate::app_window::theme::resolve(config.read(cx).config(), workspace.system_dark);
        let overridden =
            resolved.palette_for_tab(workspace.tabs.get(1).expect("first tab").saved());
        let forest = tterm_core::theme::definitions::get_preset_theme("forest")
            .expect("the forest theme exists")
            .terminal
            .to_terminal_palette();
        assert_eq!(overridden, forest);
        let first_tab = workspace.tabs.get(1).expect("first tab");
        assert_eq!(first_tab.view.read(cx).palette(), &forest);
        assert_eq!(first.session().expect("first session").palette(), forest);
        let second_tab = workspace.tabs.get(2).expect("second tab");
        assert_eq!(
            second_tab.view.read(cx).palette(),
            &resolved.terminal_palette()
        );
        assert_eq!(
            second_tab
                .binding
                .as_ref()
                .expect("binding")
                .session()
                .expect("second session")
                .palette(),
            resolved.terminal_palette()
        );
    });

    harness.visual.update(|window, _| window.remove_window());
}
