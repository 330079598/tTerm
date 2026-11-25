//! Applying the shared config font: cell metrics, PTY resize and row caches.
//!
//! Every assertion reads the view the way a window does: a real headless
//! frame, the measured cell and the grid the resize sink was told. The model
//! stands in for a live session, so the tests also prove that a font change
//! keeps its content and history rather than replacing the session.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Entity, TestAppContext, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};

use super::TerminalView;
use super::TerminalViewConfig;
use super::font::TerminalFont;

/// The constants the tests change to; 14 is the platform default.
const DEFAULT_SIZE: f32 = 14.0;
const LARGER_SIZE: u16 = 28;

struct Harness {
    window: AnyWindowHandle,
    view: Entity<TerminalView>,
    model: Arc<Terminal>,
    resizes: Rc<RefCell<Vec<(GridSize, CellSize)>>>,
}

impl Harness {
    fn new(cx: &mut TestAppContext) -> Self {
        Self::with_font(cx, TerminalFont::platform_default())
    }

    fn with_font(cx: &mut TestAppContext, font: TerminalFont) -> Self {
        cx.update(gpui_kit::init);
        let model = Arc::new(Terminal::new(
            Config {
                scrollback: 200,
                ..Config::default()
            },
            GridSize::new(80, 24),
            CellSize::new(8, 16),
        ));
        let resizes = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&resizes);
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(800.0), px(600.0)),
        };
        let (window, view) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..WindowOptions::default()
                },
                cx,
                |_, cx| {
                    cx.new(|cx| {
                        TerminalView::new(
                            cx,
                            Arc::clone(&model),
                            TerminalViewConfig {
                                font,
                                resize: Some(Rc::new(move |grid, cell| {
                                    sink.borrow_mut().push((grid, cell));
                                })),
                                ..TerminalViewConfig::default()
                            },
                        )
                    })
                },
            )
            .expect("open the terminal test window")
        });
        let harness = Self {
            window,
            view,
            model,
            resizes,
        };
        harness.draw(cx);
        harness
    }

    fn update<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut TerminalView, &mut Window, &mut Context<TerminalView>) -> R,
    ) -> R {
        cx.update_window(self.window, |_, window, cx| {
            self.view.update(cx, |view, cx| f(view, window, cx))
        })
        .expect("update the terminal view")
    }

    fn draw(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, window, cx| {
            window.render_frame(cx);
        })
        .expect("draw the terminal view");
    }

    fn output(&self, cx: &mut TestAppContext, bytes: &[u8]) {
        self.model.write(bytes);
        self.update(cx, |view, _, cx| view.notify_output(cx));
        self.draw(cx);
    }

    fn apply(&self, cx: &mut TestAppContext, family: &str, size: u16) -> bool {
        self.update(cx, |view, _, cx| view.set_shared_font(family, size, cx))
    }

    fn font(&self, cx: &mut TestAppContext) -> TerminalFont {
        self.view.read_with(cx, |view, _| view.font().clone())
    }

    fn measurements(&self, cx: &mut TestAppContext) -> u64 {
        self.view
            .read_with(cx, |view, _| view.stats().font_measurements)
    }

    fn rows_laid_out(&self, cx: &mut TestAppContext) -> u64 {
        self.view
            .read_with(cx, |view, _| view.stats().rows_laid_out)
    }

    fn cache_misses(&self, cx: &mut TestAppContext) -> u64 {
        self.view.read_with(cx, |view, _| view.stats().cache_misses)
    }

    fn grid(&self, cx: &mut TestAppContext) -> GridSize {
        self.view
            .read_with(cx, |view, _| view.grid_size())
            .expect("a prepared grid")
    }

    fn resized(&self) -> usize {
        self.resizes.borrow().len()
    }

    fn last_resize(&self) -> (GridSize, CellSize) {
        *self.resizes.borrow().last().expect("a resize request")
    }
}

fn visible_text(model: &Terminal) -> Vec<String> {
    model
        .snapshot()
        .rows()
        .iter()
        .map(|row| {
            row.cells()
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

#[gpui_kit::test]
fn a_shared_font_change_re_measures_the_grid_and_resizes_the_pty(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    h.output(cx, b"history-line\r\n");
    let before = h.view.read_with(cx, |view, _| {
        (
            view.cell_geometry().expect("measured metrics"),
            view.grid_size().expect("prepared grid"),
        )
    });
    let resizes_before = h.resized();
    let measurements_before = h.measurements(cx);
    let misses_before = h.cache_misses(cx);
    let laid_before = h.rows_laid_out(cx);
    let scrollback_before = h.model.scrollback();

    assert!(h.apply(cx, r#""Fira Code", monospace"#, LARGER_SIZE));

    let after = h.view.read_with(cx, |view, _| {
        view.cell_geometry().expect("measured metrics")
    });
    assert_eq!(after.font_size, f32::from(LARGER_SIZE));
    assert_eq!(after.scale_factor, before.0.scale_factor);
    assert!(
        after.cell_height > before.0.cell_height * 1.5,
        "a 28px font must measure taller than the 14px default: {} vs {}",
        after.cell_height,
        before.0.cell_height
    );
    assert!(after.cell_width > before.0.cell_width);

    let grid = h.grid(cx);
    assert!(grid.columns < before.1.columns);
    assert!(grid.screen_lines < before.1.screen_lines);
    assert_eq!(h.model.grid_size(), grid);

    // The session heard the new cell once, with the grid the view now draws.
    assert_eq!(h.resized(), resizes_before + 1);
    assert_eq!(h.last_resize().0, grid);
    assert_eq!(h.last_resize().1, after.model_cell_size());

    // One chain resolution and measurement for the change, not one per
    // setting or per frame.
    assert_eq!(h.measurements(cx), measurements_before + 1);

    // The cached rows carried the old glyphs, so the frame the change asked
    // for shapes every distinct row again instead of reusing one.
    assert!(
        h.cache_misses(cx) > misses_before,
        "rows shaped with the old font must not be reused"
    );
    assert!(h.rows_laid_out(cx) > laid_before);

    // The session survived: the same model still holds the same content and
    // the same retained-history capacity.
    assert!(Arc::ptr_eq(
        &h.model,
        &h.view.read_with(cx, |view, _| view.model())
    ));
    assert_eq!(h.model.scrollback(), scrollback_before);
    assert_eq!(
        visible_text(&h.model).first().map(String::as_str),
        Some("history-line")
    );
}

#[gpui_kit::test]
fn an_unchanged_shared_font_neither_measures_nor_resizes(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let resizes_before = h.resized();
    let measurements_before = h.measurements(cx);

    assert!(h.apply(cx, "Menlo, monospace", 18));
    let resizes_after_change = h.resized();
    let measurements_after_change = h.measurements(cx);
    assert_eq!(resizes_after_change, resizes_before + 1);
    assert_eq!(measurements_after_change, measurements_before + 1);

    assert!(!h.apply(cx, "Menlo, monospace", 18));
    assert_eq!(h.grid(cx), h.model.grid_size());
    assert_eq!(h.resized(), resizes_after_change);
    assert_eq!(h.measurements(cx), measurements_after_change);
    assert_eq!(h.font(cx).families, ["Menlo", "monospace"]);
}

#[gpui_kit::test]
fn drawing_more_frames_does_not_measure_the_font_again(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let measurements = h.measurements(cx);
    let resizes = h.resized();
    for _ in 0..4 {
        h.output(cx, b"frame\r\n");
        h.draw(cx);
    }
    assert_eq!(
        h.measurements(cx),
        measurements,
        "font resolution belongs to a font or scale change, not to a frame"
    );
    assert_eq!(h.resized(), resizes);
}

#[gpui_kit::test]
fn a_hidden_tab_applies_the_font_and_resizes_when_it_is_shown(cx: &mut TestAppContext) {
    let h = Harness::new(cx);
    let grid_before = h.grid(cx);
    h.update(cx, |view, _, cx| view.set_visible(false, cx));
    let resizes_before = h.resized();

    assert!(h.apply(cx, "Menlo, monospace", 22));
    assert_eq!(h.font(cx).size, 22.0);
    assert!(
        h.view
            .read_with(cx, |view, _| view.cell_geometry())
            .is_none(),
        "a hidden tab keeps the last metrics until it is laid out again"
    );
    assert_eq!(
        h.resized(),
        resizes_before,
        "a hidden tab must not resize a PTY the user cannot see"
    );

    h.update(cx, |view, _, cx| view.set_visible(true, cx));
    h.draw(cx);
    let grid = h.grid(cx);
    assert_ne!(grid, grid_before);
    assert_eq!(h.model.grid_size(), grid);
    assert_eq!(h.resized(), resizes_before + 1);
}

#[gpui_kit::test]
fn a_new_view_starts_with_the_shared_font(cx: &mut TestAppContext) {
    let font = TerminalFont::from_shared(r#""IBM Plex Mono", monospace"#, 16);
    let h = Harness::with_font(cx, font);

    let applied = h.font(cx);
    assert_eq!(applied.families, ["IBM Plex Mono", "monospace"]);
    assert_eq!(applied.size, 16.0);
    let metrics = h.view.read_with(cx, |view, _| {
        view.cell_geometry().expect("measured metrics")
    });
    assert_eq!(metrics.font_size, 16.0);
    assert_ne!(metrics.font_size, DEFAULT_SIZE);
}
