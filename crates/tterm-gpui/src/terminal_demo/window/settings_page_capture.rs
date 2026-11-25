//! Headless production screenshots of the settings route.
//!
//! The capture uses `HeadlessAppContext` with the real platform text system
//! and the platform headless renderer, the same renderer the parity toolchain
//! uses. It mounts the production `SettingsPage`, `ScrollbackSetting` and
//! input entities, drives them with real key events and writes 800x600 logical
//! pixels at 2x. No desktop window opens and no screen-recording permission is
//! needed. The configuration uses in-memory defaults and no-op writers, so the
//! command never reads or writes a user config or database.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Entity, HeadlessAppContext, IntoElement, Keystroke, Modifiers, Render, Window, px,
    size,
};
use tterm_core::config::AppConfig;
use tterm_core::i18n::language::Language;

use super::background;
use super::settings_page::SettingsPage;
use crate::config_state::{ConfigState, PaneTemplatesWriter, ScrollbackWriter};
use crate::local_shells_setting::LocalShellsSetting;
use crate::locale::{Locale, LocaleGlobal};
use crate::scrollback_setting::ScrollbackSetting;

const VIEWPORT: (u32, u32) = (800, 600);
/// The reference capture is stored at this pixel density.
const SCALE: f32 = 2.0;

struct CaptureView {
    page: Entity<SettingsPage>,
}

impl Render for CaptureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.page
            .update(cx, |page, cx| page.render_page(window, cx))
    }
}

/// Captures the production settings page and measures it against `reference`.
pub(crate) fn capture_headlessly(output: &Path, reference: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the capture runtime: {error}"))?;
    let runtime_handle = runtime.handle().clone();
    let theme = background::theme(None);
    let mut context = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(()),
        gpui_kit::platform::current_headless_renderer,
    );
    context.update(|cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
    });

    let mut entities = None;
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
                    cx.new(|cx| ScrollbackSetting::new(config.clone(), locale.clone(), theme, cx));
                let local_shells =
                    cx.new(|cx| LocalShellsSetting::new(config, locale.clone(), theme, cx));
                let page = cx.new(|cx| {
                    SettingsPage::new(locale.clone(), theme, scrollback, local_shells, cx)
                });
                page.update(cx, |page, cx| {
                    page.set_font_system_fonts(
                        vec!["SF Mono".into(), "Menlo".into(), "JetBrains Mono".into()],
                        cx,
                    );
                    page.open(window, cx);
                });
                entities = Some((page.clone(), locale.clone()));
                cx.new(|_| CaptureView { page })
            },
        )
        .map_err(|error| format!("could not open the headless settings window: {error}"))?;
    let (_, locale) = entities.ok_or("the page entities were not built with the window")?;

    context
        .update_window(window.into(), |_, window, cx| {
            window.set_scale_factor(SCALE);
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not render the capture window: {error}"))?;

    let mut screens = serde_json::Map::new();
    capture_terminal_page(&mut context, window.into(), output, &mut screens)?;
    type_text(&mut context, window.into(), "scrollback")?;
    capture(
        &mut context,
        window.into(),
        output,
        &mut screens,
        "settings.search.scrollback",
    )?;

    press(&mut context, window.into(), "enter")?;
    focus_search(&mut context, window.into())?;
    capture(
        &mut context,
        window.into(),
        output,
        &mut screens,
        "settings.terminal.scrollback",
    )?;

    type_text(&mut context, window.into(), "zzz")?;
    capture(
        &mut context,
        window.into(),
        output,
        &mut screens,
        "settings.search.empty",
    )?;
    press(&mut context, window.into(), "escape")?;

    context.update(|cx| {
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });
    type_text(&mut context, window.into(), "回滚")?;
    capture(
        &mut context,
        window.into(),
        output,
        &mut screens,
        "settings.search.scrollback-zh",
    )?;
    press(&mut context, window.into(), "enter")?;
    focus_search(&mut context, window.into())?;
    capture(
        &mut context,
        window.into(),
        output,
        &mut screens,
        "settings.terminal.scrollback-zh",
    )?;

    let measurements = measure_against_reference(output, reference, &screens)?;
    let manifest = serde_json::json!({
        "version": 1,
        "method": "gpui_kit::HeadlessAppContext::with_platform + current_platform(true); production SettingsPage and real key events",
        "isolation": "in-memory AppConfig with no-op writers; no user config, database or credential file is read",
        "theme": "dark",
        "uiStyle": "classic",
        "uiScalePercent": 100,
        "viewport": { "width": VIEWPORT.0, "height": VIEWPORT.1, "deviceScaleFactor": SCALE },
        "titleBarHeight": 0,
        "crop": "none; the production page already occupies the full 800x600 content area",
        "screens": screens,
        "measurements": measurements,
    });
    std::fs::write(
        output.join("capture.json"),
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("could not write the capture manifest: {error}"))?;
    Ok(())
}

fn capture_terminal_page(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    output: &Path,
    screens: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    capture(context, window, output, screens, "settings.terminal.font")?;
    scroll_content(context, window, -3000.0)?;
    capture(
        context,
        window,
        output,
        screens,
        "settings.terminal.preview",
    )?;
    scroll_content(context, window, 3000.0)
}

fn scroll_content(
    context: &mut HeadlessAppContext,
    handle: gpui_kit::AnyWindowHandle,
    delta_y: f32,
) -> Result<(), String> {
    use gpui_kit::{InputEvent, ScrollDelta, ScrollEvent, ScrollSource, TouchPhase, point};

    context
        .update_window(handle, |_, window, cx| {
            let event = ScrollEvent {
                position: point(px(746.0), px(120.0)),
                delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
                modifiers: Default::default(),
                touch_phase: TouchPhase::Moved,
                source: ScrollSource::Pointer,
            };
            window.dispatch_event(event.to_platform_input(), cx);
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not scroll the settings content: {error}"))?;
    context.run_until_parked();
    Ok(())
}

fn focus_search(
    context: &mut HeadlessAppContext,
    handle: gpui_kit::AnyWindowHandle,
) -> Result<(), String> {
    use gpui_kit::{InputEvent, MouseButton, MouseDownEvent, MouseUpEvent, point};
    context
        .update_window(handle, |_, window, cx| {
            let position = point(px(640.0), px(40.0));
            let _ = window.draw(cx);
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            };
            let up = MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            };
            window.dispatch_event(down.to_platform_input(), cx);
            window.dispatch_event(up.to_platform_input(), cx);
        })
        .map_err(|error| format!("could not focus the settings search: {error}"))?;
    context.run_until_parked();
    Ok(())
}

fn press(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    chord: &'static str,
) -> Result<(), String> {
    let keystroke = Keystroke::parse(chord)
        .map_err(|error| format!("could not parse the capture chord {chord}: {error}"))?;
    context
        .update_window(window, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .map_err(|error| format!("the capture window rejected a key: {error}"))?;
    context.run_until_parked();
    Ok(())
}

fn type_text(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    text: &str,
) -> Result<(), String> {
    for character in text.chars() {
        let key = character.to_string();
        let keystroke = Keystroke {
            modifiers: Modifiers::default(),
            key: key.clone(),
            key_char: Some(key),
        };
        context
            .update_window(window, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .map_err(|error| format!("the capture window rejected text: {error}"))?;
    }
    context.run_until_parked();
    Ok(())
}

fn capture(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    output: &Path,
    screens: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<(), String> {
    // The focused input hides its caret in the reference capture. Advancing
    // the simulated clock past the blink interval does the same here.
    context.advance_clock(Duration::from_millis(750));
    context
        .update_window(window, |_, window, cx| {
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not render {name}: {error}"))?;
    context.run_until_parked();
    let image = context
        .capture_screenshot(window)
        .map_err(|error| format!("could not capture {name}: {error}"))?;
    let expected = (VIEWPORT.0 * SCALE as u32, VIEWPORT.1 * SCALE as u32);
    if image.dimensions() != expected {
        return Err(format!(
            "{name} rendered at {}x{}, expected {}x{} at {SCALE}x",
            image.width(),
            image.height(),
            expected.0,
            expected.1
        ));
    }
    let path = output.join(format!("{name}.png"));
    image
        .save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    screens.insert(
        name.to_owned(),
        serde_json::json!({
            "image": format!("{name}.png"),
            "width": image.width(),
            "height": image.height(),
        }),
    );
    Ok(())
}

type Rect = (f64, f64, f64, f64);
type Region = (&'static str, Rect, Rect);
type ScreenCase = (&'static str, &'static str, &'static [Region]);

const HEADER: (&str, Rect, Rect) = ("header", (0.0, 0.0, 800.0, 73.0), (0.0, 0.0, 800.0, 73.0));
const SEARCH_RESULT: (&str, Rect, Rect) = (
    "search-result",
    (192.0, 73.0, 608.0, 120.0),
    (192.0, 73.0, 608.0, 120.0),
);
const SCROLLBACK_FIELD: (&str, Rect, Rect) = (
    "scrollback-field",
    (216.0, 281.0, 536.0, 111.0),
    (216.0, 162.0, 536.0, 111.0),
);

fn measure_against_reference(
    output: &Path,
    reference: &Path,
    screens: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    let cases: [ScreenCase; 4] = [
        (
            "settings.search.scrollback",
            "settings.search.scrollback",
            &[HEADER, SEARCH_RESULT],
        ),
        (
            "settings.terminal.scrollback",
            "settings.terminal.scrollback",
            &[HEADER, SCROLLBACK_FIELD],
        ),
        (
            "settings.search.scrollback-zh",
            "settings.search.scrollback-zh",
            &[HEADER, SEARCH_RESULT],
        ),
        (
            "settings.terminal.scrollback-zh",
            "settings.search.scrollback.open-zh",
            &[HEADER, SCROLLBACK_FIELD],
        ),
    ];
    let mut measurements = Vec::new();
    for (actual_name, reference_name, regions) in cases {
        if !screens.contains_key(actual_name) {
            continue;
        }
        let actual_path = output.join(format!("{actual_name}.png"));
        let reference_path = reference.join(format!("{reference_name}.png"));
        let actual = image::open(&actual_path)
            .map_err(|error| format!("could not read {}: {error}", actual_path.display()))?
            .to_rgba8();
        let reference = image::open(&reference_path)
            .map_err(|error| format!("could not read {}: {error}", reference_path.display()))?
            .to_rgba8();
        for (region, reference_bounds, actual_bounds) in regions {
            measurements.push(measure_region(
                actual_name,
                region,
                &actual,
                &reference,
                *reference_bounds,
                *actual_bounds,
            )?);
        }
    }
    Ok(measurements)
}

fn measure_region(
    screen: &str,
    region: &str,
    actual: &image::RgbaImage,
    reference: &image::RgbaImage,
    reference_bounds: Rect,
    actual_bounds: Rect,
) -> Result<serde_json::Value, String> {
    let scale = actual.width() as f64 / f64::from(VIEWPORT.0);
    if actual.dimensions() != reference.dimensions() {
        return Err(format!(
            "{screen} reference is {:?}, capture is {:?}",
            reference.dimensions(),
            actual.dimensions()
        ));
    }
    let pixels = |bounds: Rect| {
        (
            (bounds.0 * scale).round() as u32,
            (bounds.1 * scale).round() as u32,
            (bounds.2 * scale).round() as u32,
            (bounds.3 * scale).round() as u32,
        )
    };
    let (actual_x, actual_y, width, height) = pixels(actual_bounds);
    let (reference_x, reference_y, reference_width, reference_height) = pixels(reference_bounds);
    if (width, height) != (reference_width, reference_height) {
        return Err(format!(
            "{screen}/{region} reference region is {reference_width}x{reference_height}, capture region is {width}x{height}"
        ));
    }
    if actual_x + width > actual.width()
        || actual_y + height > actual.height()
        || reference_x + width > reference.width()
        || reference_y + height > reference.height()
    {
        return Err(format!("{screen}/{region} region is outside its image"));
    }
    let mut absolute_error = 0_u64;
    let mut changed = 0_u64;
    let mut max_error = 0_u8;
    for y in 0..height {
        for x in 0..width {
            let reference_pixel = reference.get_pixel(reference_x + x, reference_y + y);
            let actual_pixel = actual.get_pixel(actual_x + x, actual_y + y);
            let mut pixel_changed = false;
            for channel in 0..3 {
                let difference = reference_pixel[channel].abs_diff(actual_pixel[channel]);
                absolute_error += u64::from(difference);
                max_error = max_error.max(difference);
                pixel_changed |= difference > 2;
            }
            changed += u64::from(pixel_changed);
        }
    }
    let count = u64::from(width) * u64::from(height);
    Ok(serde_json::json!({
        "screen": screen,
        "region": region,
        "referenceBounds": {
            "x": reference_bounds.0,
            "y": reference_bounds.1,
            "width": reference_bounds.2,
            "height": reference_bounds.3,
        },
        "actualBounds": {
            "x": actual_bounds.0,
            "y": actual_bounds.1,
            "width": actual_bounds.2,
            "height": actual_bounds.3,
        },
        "pixels": count,
        "meanAbsoluteError": absolute_error as f64 / (count as f64 * 3.0),
        "maxChannelError": max_error,
        "pixelsAbove2": changed,
        "pixelFractionAbove2": changed as f64 / count as f64,
    }))
}
