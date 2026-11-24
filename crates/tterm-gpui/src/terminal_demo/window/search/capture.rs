//! Headless screenshots of the production search bar.
//!
//! Debug builds with the automation feature only. The window uses the real
//! platform text system and headless renderer, and the model receives
//! synthetic output, so no shell, SSH connection or user file is involved.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Entity, HeadlessAppContext, IntoElement, Keystroke, Modifiers, Render, Window, px,
    size,
};
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::SearchBar;
use crate::app_window::background;
use crate::locale::{Locale, LocaleGlobal};

const VIEWPORT: (u32, u32) = (800, 600);
const SCALE: f32 = 2.0;

struct CaptureRoot {
    view: Entity<TerminalView>,
    bar: Entity<SearchBar>,
}

impl Render for CaptureRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::div()
            .size_full()
            .relative()
            .child(self.view.clone())
            .child(self.bar.clone())
    }
}

/// Renders the bar over a synthetic scrolled-back terminal and writes `search-bar.png`.
pub(crate) fn capture_search_headlessly(output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;
    let theme = background::theme(None);
    let mut context = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(()),
        gpui_kit::platform::current_headless_renderer,
    );
    context.update(|cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });

    let model = Arc::new(Terminal::new(
        Config {
            scrollback: 2_000,
            ..Config::default()
        },
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    for index in 0..80 {
        let line = if index % 6 == 0 {
            format!("\x1b[33mneedle\x1b[0m line-{index:03} 演示搜索\r\n")
        } else {
            format!("output line-{index:03} with a needle\r\n")
        };
        model.write(line.as_bytes());
    }
    model.scroll(tterm_terminal::terminal::Scroll::Delta(30));

    let window = context
        .open_window(
            size(px(VIEWPORT.0 as f32), px(VIEWPORT.1 as f32)),
            |window, cx| {
                let locale = cx.new(|_| Locale::new(Language::En));
                LocaleGlobal::set(cx, locale.clone());
                let view = cx.new(|cx| {
                    TerminalView::new(cx, Arc::clone(&model), TerminalViewConfig::default())
                });
                let bar = cx.new(|cx| {
                    SearchBar::new(
                        cx,
                        window,
                        Arc::clone(&model),
                        view.downgrade(),
                        locale,
                        theme,
                    )
                });
                window.focus(&view.read(cx).focus_handle(), cx);
                bar.update(cx, |bar, cx| bar.open(window, cx));
                cx.new(|_| CaptureRoot { view, bar })
            },
        )
        .map_err(|error| format!("could not open the headless search window: {error}"))?;

    for character in "needle".chars() {
        let key = character.to_string();
        let keystroke = Keystroke {
            modifiers: Modifiers::default(),
            key: key.clone(),
            key_char: Some(key),
        };
        context
            .update_window(window.into(), |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .map_err(|error| format!("the search input rejected a key: {error}"))?;
    }
    context.advance_clock(Duration::from_millis(250));
    context.run_until_parked();
    context
        .update_window(window.into(), |_, window, cx| {
            window.set_scale_factor(SCALE);
            let _ = window.draw(cx);
        })
        .map_err(|error| format!("could not render the search capture: {error}"))?;
    context.run_until_parked();
    let image = context
        .capture_screenshot(window.into())
        .map_err(|error| format!("could not capture the search bar: {error}"))?;
    let expected = (VIEWPORT.0 * SCALE as u32, VIEWPORT.1 * SCALE as u32);
    if image.dimensions() != expected {
        return Err(format!(
            "the search capture rendered at {}x{}, expected {}x{}",
            image.width(),
            image.height(),
            expected.0,
            expected.1
        ));
    }
    let path = output.join("search-bar.png");
    image
        .save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    std::fs::write(
        output.join("capture.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "version": 1,
            "method": "gpui_kit::HeadlessAppContext::with_platform + current_platform(true); production TerminalView and SearchBar with real key events",
            "isolation": "synthetic in-process output; no shell, SSH connection, user config or credential file",
            "theme": "dark",
            "language": "en",
            "viewport": { "width": VIEWPORT.0, "height": VIEWPORT.1, "deviceScaleFactor": SCALE },
            "image": "search-bar.png",
            "state": "query=needle, first active match, overview ruler marks visible"
        }))
        .map_err(|error| error.to_string())?
            + "\n",
    )
    .map_err(|error| format!("could not write the capture manifest: {error}"))?;
    Ok(())
}
