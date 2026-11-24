//! Paired headless screenshots and measurements of the production search bar.

use std::{cell::Cell, path::Path, rc::Rc, sync::Arc, time::Duration};

use gpui_kit::prelude::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    App, Bounds, Context, Entity, Focusable as _, HeadlessAppContext, IntoElement, Pixels, Render,
    Window, WindowHandle, point, px, size,
};
use serde_json::{Value, json};
use tterm_core::i18n::language::Language;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Scroll, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::SearchBar;
use crate::app_window::background;
use crate::locale::{Locale, LocaleGlobal};

const VIEWPORT: (u32, u32) = (800, 600);
const SCALE: f32 = 2.0;
const REFERENCE_ACTIVE: usize = 27;
const REFERENCE_TOP: usize = 14;

struct CaptureRoot {
    view: Entity<TerminalView>,
    bar: Entity<SearchBar>,
    bar_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Render for CaptureRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let measured = self.bar_bounds.clone();
        gpui_kit::div()
            .on_children_prepainted(move |children, _, _| measured.set(children.get(1).copied()))
            .size_full()
            .relative()
            .child(self.view.clone())
            .child(self.bar.clone())
    }
}

fn build(window: &mut Window, cx: &mut App) -> Entity<CaptureRoot> {
    window.set_scale_factor(SCALE);
    let model = Arc::new(Terminal::new(
        Config {
            scrollback: 2_000,
            ..Config::default()
        },
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    let locale = cx.new(|_| Locale::new(Language::En));
    LocaleGlobal::set(cx, locale.clone());
    let view =
        cx.new(|cx| TerminalView::new(cx, Arc::clone(&model), TerminalViewConfig::default()));
    let bar = cx.new(|cx| {
        SearchBar::new(
            cx,
            window,
            model,
            view.downgrade(),
            locale,
            background::theme(None),
        )
    });
    cx.new(|_| CaptureRoot {
        view,
        bar,
        bar_bounds: Rc::default(),
    })
}

fn frame<T>(
    context: &mut HeadlessAppContext,
    handle: WindowHandle<CaptureRoot>,
    action: impl FnOnce(&CaptureRoot, &mut Window, &mut App) -> Result<T, String>,
) -> Result<T, String> {
    context
        .update_window(handle.into(), |root, window, cx| {
            let root = root
                .downcast::<CaptureRoot>()
                .map_err(|_| "unexpected screenshot root")?;
            root.update(cx, |root, cx| action(root, window, cx))
        })
        .map_err(|error| error.to_string())?
}

fn prepare(
    context: &mut HeadlessAppContext,
    handle: WindowHandle<CaptureRoot>,
) -> Result<(), String> {
    frame(context, handle, |root, window, cx| {
        let model = root.view.read(cx).model();
        for index in 0..80 {
            let line = if index % 6 == 0 {
                format!("\x1b[33mneedle\x1b[0m line-{index:03} 演示搜索\r\n")
            } else {
                format!("output line-{index:03} with a needle\r\n")
            };
            model.write(line.as_bytes());
        }
        model.scroll(Scroll::Delta(30));
        root.view.update(cx, |view, cx| view.notify_output(cx));
        root.bar.update(cx, |bar, cx| bar.open(window, cx));
        Ok(())
    })?;
    context
        .update_window(handle.into(), |_, window, cx| window.input("needle", cx))
        .map_err(|error| error.to_string())?;
    context.advance_clock(Duration::from_millis(250));
    context.run_until_parked();
    let presses = frame(context, handle, |root, _, cx| {
        let bar = root.bar.read(cx);
        let count = bar.result.as_ref().map_or(0, |result| result.match_count());
        if count != 80 {
            return Err(format!("expected 80 matches, found {count}"));
        }
        Ok((REFERENCE_ACTIVE + count - bar.active.ok_or("no active match")?) % count)
    })?;
    for _ in 0..presses {
        context
            .update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .map_err(|error| error.to_string())?;
    }
    frame(context, handle, |root, _, cx| {
        let model = root.view.read(cx).model();
        let top = root.view.read(cx).search_origin().line;
        model.scroll(Scroll::Delta(
            i32::try_from(top - REFERENCE_TOP as i64).map_err(|error| error.to_string())?,
        ));
        root.view.update(cx, |view, cx| view.notify_output(cx));
        Ok(())
    })
}

fn rect(bounds: Bounds<Pixels>) -> Value {
    json!({"x": f32::from(bounds.left()), "y": f32::from(bounds.top()), "width": f32::from(bounds.size.width), "height": f32::from(bounds.size.height)})
}

fn state(root: &CaptureRoot, window: &mut Window, cx: &mut App) -> Result<Value, String> {
    let view = root.view.read(cx);
    let bar = root.bar.read(cx);
    let matched = view
        .search_overlay()
        .active_match()
        .ok_or("no highlighted match")?;
    let focused = bar.input.read(cx).focus_handle(cx).is_focused(window);
    let snapshot = bar.model.snapshot();
    let top = view.search_origin().line;
    if !bar.is_open()
        || bar.query != "needle"
        || !focused
        || bar.drag.is_some()
        || bar.active != Some(REFERENCE_ACTIVE)
        || top != REFERENCE_TOP as i64
    {
        return Err(format!(
            "unexpected capture: query={}, focus={focused}, active={:?}, viewportY={top}, dragging={}",
            bar.query,
            bar.active,
            bar.drag.is_some()
        ));
    }
    Ok(json!({
        "method": "gpui-kit headless renderer, production SearchBar and TerminalView; synthetic 80-line model, no shell",
        "viewport": {"width": f32::from(window.viewport_size().width), "height": f32::from(window.viewport_size().height), "deviceScaleFactor": window.scale_factor()},
        "theme": "dark", "language": "en", "focus": "search-input", "query": bar.query, "searchOpen": bar.is_open(), "dragging": bar.drag.is_some(),
        "options": {"caseSensitive": bar.options.case_sensitive, "wholeWord": bar.options.whole_word, "regex": bar.options.regex},
        "result": {"index": bar.active, "count": bar.result.as_ref().map(|result| result.match_count())},
        "text": {"count": bar.count_text(cx), "input": bar.input.read(cx).value().to_string()},
        "bounds": {"bar": rect(root.bar_bounds.get().ok_or("missing bar frame bounds")?), "surface": rect(view.bounds().ok_or("missing terminal frame bounds")?)},
        "terminal": {"cols": snapshot.grid().columns, "rows": snapshot.grid().screen_lines, "baseY": snapshot.history_size(), "viewportY":top,
            "activeMatch": {"line":matched.start.line,"firstColumn":matched.start.column,"lastColumnInclusive":matched.end.column},
            "visibleSearchSpans":view.stats().search_spans,"visibleActiveSpans":view.stats().search_active_spans,"overviewMarks":view.stats().overview_marks}
    }))
}

fn record(
    context: &mut HeadlessAppContext,
    handle: WindowHandle<CaptureRoot>,
    name: &str,
    output: &Path,
) -> Result<(), String> {
    context
        .update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .map_err(|error| error.to_string())?;
    let measured = frame(context, handle, |root, window, cx| {
        if name == "dragged" {
            let bar = root.bar_bounds.get().ok_or("missing dragged bar bounds")?;
            let surface = root
                .view
                .read(cx)
                .bounds()
                .ok_or("missing surface bounds")?;
            if surface.right() - bar.right() != px(8.) || surface.bottom() - bar.bottom() != px(8.)
            {
                return Err(format!(
                    "drag did not reach the lower-right margin: {bar:?} in {surface:?}"
                ));
            }
        }
        state(root, window, cx)
    })?;
    let image = context
        .capture_screenshot(handle.into())
        .map_err(|error| error.to_string())?;
    if image.dimensions() != (VIEWPORT.0 * SCALE as u32, VIEWPORT.1 * SCALE as u32) {
        return Err(format!("unexpected capture size: {:?}", image.dimensions()));
    }
    image
        .save(output.join(format!("terminal.search.{name}.png")))
        .map_err(|error| error.to_string())?;
    std::fs::write(
        output.join(format!("terminal.search.{name}.state.json")),
        serde_json::to_string_pretty(&measured).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| error.to_string())
}

/// Captures the reference query before and after a real grip drag to the lower right.
pub(crate) fn capture_search_headlessly(output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output)
        .map_err(|error| format!("could not create {}: {error}", output.display()))?;
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
    let handle = context
        .open_window(size(px(VIEWPORT.0 as f32), px(VIEWPORT.1 as f32)), build)
        .map_err(|error| error.to_string())?;
    context
        .update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .map_err(|error| error.to_string())?;
    prepare(&mut context, handle)?;
    record(&mut context, handle, "default", output)?;
    let from = frame(&mut context, handle, |root, _, _| {
        Ok(root.bar_bounds.get().ok_or("missing bar bounds")?.origin + point(px(18.), px(22.)))
    })?;
    context
        .update_window(handle.into(), |_, window, cx| {
            window.drag(from, point(px(799.), px(599.)), cx)
        })
        .map_err(|error| error.to_string())?;
    record(&mut context, handle, "dragged", output)
}
