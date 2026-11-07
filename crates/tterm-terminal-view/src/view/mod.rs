//! The GPUI terminal element: measurement, layout, painting and scrolling.
//!
//! [`TerminalView`] owns the view-side state: font metrics, the row layout
//! cache, the blink timer, visibility and the wheel accumulator. The model
//! ([`tterm_terminal::terminal::Terminal`]) is shared with the session and is
//! only locked while a snapshot is copied.
//!
//! [`element::TerminalElement`] is the custom [`gpui_kit::Element`]. Its prepaint
//! takes the snapshot and builds the rows; its paint only draws the prepared
//! frame. A frame has three entry points:
//!
//! - output calls [`TerminalView::notify_output`], which asks for one frame;
//! - a blink timer asks for one frame while the cursor blinks;
//! - nothing at all otherwise, so an idle terminal draws no frames.

mod blink;
mod cache;
mod clipboard;
mod composition;
pub mod element;
pub mod font;
mod frame;
mod ime;
mod input_handler;
mod keyboard;
mod paint;
mod paste;
mod pointer;
mod prepare;
mod row;
mod scroll;
mod style;
mod wheel;

#[cfg(test)]
mod ime_tests;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod pointer_tests;
#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    Bounds, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    Render, Styled, Task, Window, div,
};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::mode::Modes;
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::terminal::Terminal;

use crate::input::keyboard::EncoderConfig;

use self::blink::BlinkState;
use self::cache::RowCache;
use self::element::TerminalElement;
use self::font::{CellGeometry, FontSet, TerminalFont};
use self::frame::FrameData;
use self::scroll::ScrollAccumulator;

/// Where bytes the view produces, such as alternate-screen wheel arrows,
/// are written.
pub type InputSink = Rc<dyn Fn(&[u8])>;

/// Reports a grid change to the session so it can resize the PTY.
pub type ResizeSink = Rc<dyn Fn(GridSize, CellSize)>;

/// The settings one terminal view starts with.
#[derive(Clone, Default)]
pub struct TerminalViewConfig {
    /// Font family chain, size, line height and letter spacing.
    pub font: TerminalFont,
    /// The theme palette.
    pub palette: TerminalPalette,
    /// Ordered byte sink shared by keyboard, clipboard and text input.
    pub input: Option<InputSink>,
    /// Alt/Option policy passed to the existing keyboard encoder.
    pub keyboard: EncoderConfig,
    /// Reports a grid change to the session so it can resize the PTY.
    pub resize: Option<ResizeSink>,
}

/// Counters the element keeps for tests and the drawing benchmark.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewStats {
    /// Frames prepared, including blink-only frames.
    pub frames: u64,
    /// Rows that were planned and shaped.
    pub rows_laid_out: u64,
    /// Row cache hits.
    pub cache_hits: u64,
    /// Row cache misses.
    pub cache_misses: u64,
    /// Rows reused because Alacritty did not damage them.
    pub damage_reuses: u64,
    /// Output notifications received.
    pub output_notifications: u64,
    /// Notifications dropped because the view is not visible.
    pub suppressed_notifications: u64,
    /// Requests for a frame that were actually made.
    pub redraw_requests: u64,
    /// Cursor blink phase changes.
    pub blink_toggles: u64,
    /// Resizes sent to the model.
    pub resizes: u64,
    /// Blink-only repaints that reused the previous snapshot and rows.
    pub blink_reuses: u64,
    /// Candidate geometry changes that requested an IME coordinate update.
    pub ime_geometry_updates: u64,
}

impl ViewStats {
    /// The fraction of row lookups that hit the cache.
    pub fn cache_hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f32 / total as f32
        }
    }
}

/// The terminal view, a GPUI entity that renders one [`element::TerminalElement`].
pub struct TerminalView {
    terminal: Arc<Terminal>,
    font: TerminalFont,
    fonts: FontSet,
    palette: TerminalPalette,
    focus_handle: FocusHandle,
    input: Option<InputSink>,
    keyboard: EncoderConfig,
    input_focused: bool,
    input_observed: bool,
    input_origin: Option<Point<Pixels>>,
    pointer: pointer::PointerState,
    composition: composition::Composition,
    ime_confirm: bool,
    preedit: Option<composition::PreeditPaint>,
    ime_geometry: Option<ime::CandidateGeometry>,
    ime_hide_task: Option<Task<()>>,
    resize: Option<ResizeSink>,
    visible: bool,
    geometry: Option<CellGeometry>,
    geometry_scale: f32,
    last_grid: Option<GridSize>,
    cache: RowCache,
    cache_frame: u64,
    style_epoch: u64,
    last_frame: Option<Arc<FrameData>>,
    blink: BlinkState,
    blink_task: Option<Task<()>>,
    blink_only: bool,
    scroll: ScrollAccumulator,
    stats: ViewStats,
}

impl TerminalView {
    /// Creates the view around an existing model.
    pub fn new(
        cx: &mut Context<Self>,
        terminal: Arc<Terminal>,
        config: TerminalViewConfig,
    ) -> Self {
        let fonts = FontSet::new(&config.font);
        let focus_handle = cx.focus_handle();
        Self {
            terminal,
            font: config.font,
            fonts,
            palette: config.palette,
            focus_handle,
            input: config.input,
            keyboard: config.keyboard,
            input_focused: false,
            input_observed: false,
            input_origin: None,
            pointer: pointer::PointerState::default(),
            composition: composition::Composition::default(),
            ime_confirm: false,
            preedit: None,
            ime_geometry: None,
            ime_hide_task: None,
            resize: config.resize,
            visible: true,
            geometry: None,
            geometry_scale: 0.0,
            last_grid: None,
            cache: RowCache::new(1024),
            cache_frame: 0,
            style_epoch: 0,
            last_frame: None,
            blink: BlinkState::default(),
            blink_task: None,
            blink_only: false,
            scroll: ScrollAccumulator::default(),
            stats: ViewStats::default(),
        }
    }

    /// The focus handle the terminal element tracks.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// The view's counters.
    pub fn stats(&self) -> ViewStats {
        self.stats
    }

    /// The grid size from the last prepared frame.
    pub fn grid_size(&self) -> Option<GridSize> {
        self.last_grid
    }

    /// The current palette.
    pub fn palette(&self) -> &TerminalPalette {
        &self.palette
    }

    /// Replaces the palette and drops every cached row.
    pub fn set_palette(&mut self, palette: TerminalPalette, cx: &mut Context<Self>) {
        if self.palette == palette {
            return;
        }
        self.palette = palette;
        self.style_epoch = self.style_epoch.wrapping_add(1);
        self.cache.clear();
        self.invalidate(cx);
    }

    /// Replaces the font and measures the grid again on the next frame.
    pub fn set_font(&mut self, font: TerminalFont, cx: &mut Context<Self>) {
        if self.font == font {
            return;
        }
        self.fonts = FontSet::new(&font);
        self.font = font;
        self.geometry = None;
        self.style_epoch = self.style_epoch.wrapping_add(1);
        self.cache.clear();
        self.invalidate(cx);
    }

    /// Shows or hides the view, as a tab switch does.
    ///
    /// A hidden view keeps parsing through the model but never asks for a
    /// frame; showing it asks for exactly one frame.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        if visible {
            self.blink_only = false;
            self.request_redraw(cx);
        } else {
            self.ime_confirm = false;
            self.composition.clear();
            self.preedit = None;
            self.ime_hide_task.take();
            self.stop_blink();
            self.blink_only = false;
        }
    }

    /// Whether the view is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether the cursor blink timer is running.
    pub fn is_blinking(&self) -> bool {
        self.blink.running
    }

    /// Whether the cursor is in its visible blink phase.
    pub fn blink_on(&self) -> bool {
        self.blink.on
    }

    /// Tells the view that the model received output.
    ///
    /// This is the only output-side call: the session writes bytes into the
    /// model and then notifies the view. A hidden view drops the notification
    /// instead of asking for a frame.
    pub fn notify_output(&mut self, cx: &mut Context<Self>) {
        self.stats.output_notifications = self.stats.output_notifications.saturating_add(1);
        self.blink.reset();
        if !self.visible {
            self.stats.suppressed_notifications =
                self.stats.suppressed_notifications.saturating_add(1);
            return;
        }
        self.blink_only = false;
        self.request_redraw(cx);
    }

    /// Invalidates the screen and cancels view-local input during a session reset.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.ime_confirm = false;
        self.composition.clear();
        self.preedit = None;
        self.invalidate(cx);
    }

    fn request_redraw(&mut self, cx: &mut Context<Self>) {
        self.stats.redraw_requests = self.stats.redraw_requests.saturating_add(1);
        cx.notify();
    }

    fn invalidate(&mut self, cx: &mut Context<Self>) {
        self.blink_only = false;
        if self.visible {
            self.request_redraw(cx);
        } else {
            self.stats.suppressed_notifications =
                self.stats.suppressed_notifications.saturating_add(1);
        }
    }

    fn stop_blink(&mut self) {
        self.blink_task.take();
        self.blink.set_running(false);
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.observe_input_focus(window, cx);
        div()
            .size_full()
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_any_mouse_down(cx.listener(Self::on_pointer_down))
            .on_mouse_move(cx.listener(Self::on_pointer_move))
            .on_mouse_up(
                gpui_kit::MouseButton::Left,
                cx.listener(Self::on_pointer_up),
            )
            .on_mouse_up(
                gpui_kit::MouseButton::Right,
                cx.listener(Self::on_pointer_up),
            )
            .on_mouse_up(
                gpui_kit::MouseButton::Middle,
                cx.listener(Self::on_pointer_up),
            )
            .on_mouse_up_out(
                gpui_kit::MouseButton::Left,
                cx.listener(Self::on_pointer_up),
            )
            .on_mouse_up_out(
                gpui_kit::MouseButton::Middle,
                cx.listener(Self::on_pointer_up),
            )
            .on_mouse_up_out(
                gpui_kit::MouseButton::Right,
                cx.listener(Self::on_pointer_up),
            )
            .on_scroll(cx.listener(Self::on_wheel))
            .child(TerminalElement::new(cx.entity()))
    }
}

/// The grid origin for a viewport.
pub(crate) fn grid_origin(
    bounds: Bounds<Pixels>,
    grid: GridSize,
    metrics: &CellGeometry,
    bottom_anchored: bool,
    scale_factor: f32,
) -> Point<Pixels> {
    let snap = |value: f32| (value * scale_factor).round() / scale_factor;
    let x = snap(bounds.origin.x.as_f32());
    let extra = if bottom_anchored {
        (bounds.size.height.as_f32() - grid.screen_lines as f32 * metrics.cell_height).max(0.0)
    } else {
        0.0
    };
    Point::new(
        gpui_kit::px(x),
        gpui_kit::px(snap(bounds.origin.y.as_f32() + extra)),
    )
}

/// Whether the viewport should sit at the bottom of the element.
pub(crate) fn bottom_anchored(snapshot: &tterm_terminal::snapshot::Snapshot) -> bool {
    snapshot.bottom_line_occupied() || snapshot.modes().contains(Modes::ALT_SCREEN)
}
