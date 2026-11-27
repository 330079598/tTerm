//! Headless screenshots of the production theme-library gallery.
//!
//! The capture mounts the production `SettingsPage`, opens the Appearance page
//! and the bundled library through their real keyboard paths, filters the
//! gallery by name and writes 800x600 logical pixels at 2x. The configuration
//! uses in-memory defaults and no-op writers, so no user config, database,
//! credential or desktop appearance is read.
//!
//! The running-session half of the acceptance is the `theme_library_smoke`
//! test, which keeps the same two PTY sessions alive across the dark and light
//! selections. This example only owns the gallery screenshots, because the
//! unit-test context has no headless renderer.

use std::path::Path;
use std::sync::Arc;

use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Entity, HeadlessAppContext, InputEvent, IntoElement, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseUpEvent, Render, Window, point, px, size,
};
use tterm_core::config::AppConfig;
use tterm_core::i18n::language::Language;

use super::super::settings_page::SettingsPage;
use crate::app_window::theme;
use crate::config_state::{ConfigState, PaneTemplatesWriter, ScrollbackWriter};
use crate::local_shells_setting::LocalShellsSetting;
use crate::locale::{Locale, LocaleGlobal};
use crate::scrollback_setting::ScrollbackSetting;

const VIEWPORT: (u32, u32) = (800, 600);
const SCALE: f32 = 2.0;
const DARK_THEME: &str = "Dracula";
const LIGHT_THEME: &str = "Catppuccin Latte";

struct CaptureView {
    page: Entity<SettingsPage>,
}

impl Render for CaptureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.page
            .update(cx, |page, cx| page.render_page(window, cx))
    }
}

/// Captures the filtered gallery for one dark and one light library theme.
pub(crate) fn capture_headlessly(output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the capture runtime: {error}"))?;
    let runtime_handle = runtime.handle().clone();
    let mut context = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(()),
        gpui_kit::platform::current_headless_renderer,
    );
    context.update(|cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
    });

    let mut page_slot = None;
    let window = context
        .open_window(
            size(px(VIEWPORT.0 as f32), px(VIEWPORT.1 as f32)),
            |window, cx| {
                let locale = cx.new(|_| Locale::new(Language::En));
                LocaleGlobal::set(cx, locale.clone());
                let scrollback_writer: ScrollbackWriter = Arc::new(|_| Ok(()));
                let pane_writer: PaneTemplatesWriter = Arc::new(|_| Ok(()));
                let config = cx.new(|_| {
                    ConfigState::with_writers(
                        AppConfig::default(),
                        &runtime_handle,
                        scrollback_writer,
                        pane_writer,
                        Arc::new(|shell| {
                            Ok(
                                tterm_core::config::local_shells_file::SavedLocalShells {
                                    local_shells: vec![shell],
                                    default_shell:
                                        tterm_core::workspace::local_shells::types::DefaultShellSettings::default(),
                                },
                            )
                        }),
                    )
                });
                let scrollback =
                    cx.new(|cx| ScrollbackSetting::new(config.clone(), locale.clone(), theme::default_theme().ui, cx));
                let local_shells =
                    cx.new(|cx| LocalShellsSetting::new(config, locale.clone(), theme::default_theme().ui, cx));
                let page = cx.new(|cx| {
                    SettingsPage::new(
                        locale.clone(),
                        theme::default_theme().ui,
                        scrollback,
                        local_shells,
                        cx,
                    )
                });
                page.update(cx, |page, cx| {
                    page.set_font_system_fonts(
                        vec!["SF Mono".into(), "Menlo".into(), "JetBrains Mono".into()],
                        cx,
                    );
                    page.open(window, cx);
                });
                page_slot = Some(page.clone());
                cx.new(|_| CaptureView { page })
            },
        )
        .map_err(|error| format!("could not open the headless theme-library window: {error}"))?;
    let page = page_slot.ok_or("the settings page was not built with the window")?;

    context
        .update_window(window.into(), |_, window, cx| {
            window.set_scale_factor(SCALE);
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not render the capture window: {error}"))?;

    type_text(&mut context, window.into(), "appearance");
    press(&mut context, window.into(), "enter");
    let theme_setting = context.update(|cx| page.read(cx).theme_setting_entity());
    let mut screens = serde_json::Map::new();

    let (name, screen) = capture_gallery(
        &mut context,
        window.into(),
        page.clone(),
        theme_setting.clone(),
        output,
        DARK_THEME,
    )?;
    screens.insert(name, screen);
    press(&mut context, window.into(), "escape");
    let (name, screen) = capture_gallery(
        &mut context,
        window.into(),
        page,
        theme_setting,
        output,
        LIGHT_THEME,
    )?;
    screens.insert(name, screen);

    let manifest = serde_json::json!({
        "version": 1,
        "method": "headless GPUI production SettingsPage and bundled theme library; current_platform(true) text system and current_headless_renderer",
        "scope": "gallery screenshots only; the same running window and two PTY sessions across the dark and light selections are covered by the theme_library_smoke test",
        "isolation": "in-memory AppConfig with no-op writers; no user config, database, credential or desktop appearance is read",
        "theme": "default",
        "uiStyle": "classic",
        "uiScalePercent": 100,
        "viewport": { "width": VIEWPORT.0, "height": VIEWPORT.1, "deviceScaleFactor": SCALE },
        "titleBarHeight": 0,
        "crop": "none; the production page already occupies the full 800x600 content area",
        "screens": screens,
    });
    std::fs::write(
        output.join("capture.json"),
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("could not write the capture manifest: {error}"))?;
    Ok(())
}

fn capture_gallery(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    page: Entity<SettingsPage>,
    theme_setting: Entity<crate::theme_setting::ThemeSetting>,
    output: &Path,
    theme_name: &str,
) -> Result<(String, serde_json::Value), String> {
    let screen = format!(
        "gallery.{}.filtered",
        if theme_name == DARK_THEME {
            "dark"
        } else {
            "light"
        }
    );
    context
        .update_window(window, |_, window, cx| {
            theme_setting.update(cx, |setting, cx| setting.open_library(window, cx));
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not open the theme library: {error}"))?;
    context.run_until_parked();

    type_text(context, window, &theme_name.to_lowercase());
    click_filter(context, window, theme_name == DARK_THEME);
    press(context, window, "down");
    let resolved = theme::resolve_theme_id(
        &tterm_core::theme::catalog::load_theme_catalog()
            .map_err(|error| error.to_string())?
            .iter()
            .find(|theme| theme.name == theme_name)
            .ok_or_else(|| format!("{theme_name} is not in the bundled theme library"))?
            .id,
    )
    .ok_or_else(|| format!("{theme_name} does not resolve to a palette"))?;
    context
        .update_window(window, |_, window, cx| {
            page.update(cx, |page, cx| page.set_theme(resolved.ui, cx));
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not repaint the gallery: {error}"))?;

    context.advance_clock(std::time::Duration::from_millis(750));
    context
        .update_window(window, |_, window, cx| {
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not render {screen}: {error}"))?;
    context.run_until_parked();
    let image = context
        .capture_screenshot(window)
        .map_err(|error| format!("could not capture {screen}: {error}"))?;
    let path = output.join(format!("{screen}.png"));
    image
        .save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok((
        screen.clone(),
        serde_json::json!({
            "image": format!("{screen}.png"),
            "width": image.width(),
            "height": image.height(),
            "theme": resolved.id,
            "query": theme_name.to_lowercase(),
            "filter": if theme_name == DARK_THEME { "dark" } else { "light" },
        }),
    ))
}

fn press(context: &mut HeadlessAppContext, window: gpui_kit::AnyWindowHandle, chord: &'static str) {
    let keystroke = Keystroke::parse(chord)
        .unwrap_or_else(|error| panic!("could not parse the capture chord {chord}: {error}"));
    context
        .update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke.clone(), cx);
        })
        .ok();
    context.run_until_parked();
}

fn type_text(context: &mut HeadlessAppContext, window: gpui_kit::AnyWindowHandle, text: &str) {
    for character in text.chars() {
        let key = character.to_string();
        let keystroke = Keystroke {
            modifiers: Modifiers::default(),
            key: key.clone(),
            key_char: Some(key),
        };
        context
            .update_window(window, |_, window, cx| {
                window.dispatch_keystroke(keystroke.clone(), cx);
            })
            .ok();
    }
    context.run_until_parked();
}

/// Clicks the Dark or Light filter in the current library header.
///
/// The panel is a fixed 800x600 production page, so the filter row sits at the
/// same logical position in every capture; the screenshots make a drift
/// visible without making this capture depend on a renderer query API.
fn click_filter(context: &mut HeadlessAppContext, window: gpui_kit::AnyWindowHandle, dark: bool) {
    let position = point(px(if dark { 283.0 } else { 329.0 }), px(302.0));
    context
        .update_window(window, |_, window, cx| {
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            };
            let up = MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            };
            window.dispatch_event(down.to_platform_input(), cx);
            window.dispatch_event(up.to_platform_input(), cx);
        })
        .expect("the filter click reaches the capture window");
    context.run_until_parked();
}
