//! IME composition stays view-local and anchored independently of TUI output.

use super::{
    font::{CellGeometry, FontSet},
    style::hsla,
};
use gpui_kit::{
    App, Bounds, Pixels, Point, ShapedLine, TextAlign, TextRun, UnderlineStyle, Window, fill,
    point, px, size,
};
use std::{
    ops::Range,
    time::{Duration, Instant},
};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::{grid::GridPoint, snapshot::Snapshot};

/// A short cursor hide is treated as an intermediate TUI repaint.
pub(super) const TRANSIENT_HIDE: Duration = Duration::from_millis(500);

/// Transient text and the cursor history needed before a composition starts.
#[derive(Default)]
pub(super) struct Composition {
    pub text: String,
    pub selected: Range<usize>,
    pub anchor: Option<GridPoint>,
    pub last_visible: Option<GridPoint>,
    pub hidden_since: Option<Instant>,
    pub resting: GridPoint,
}

impl Composition {
    pub fn active(&self) -> bool {
        self.anchor.is_some()
    }
    pub fn clear(&mut self) {
        self.text.clear();
        self.selected = 0..0;
        self.anchor = None;
    }
    pub fn refresh(&mut self, snapshot: &Snapshot, now: Instant) {
        let cursor = snapshot.cursor();
        if cursor.is_visible() {
            self.last_visible = Some(cursor.point);
            self.hidden_since = None;
        } else if self.hidden_since.is_none() {
            self.hidden_since = Some(now);
        }
        self.resting = if self
            .hidden_since
            .is_some_and(|since| now.duration_since(since) < TRANSIENT_HIDE)
        {
            self.last_visible.unwrap_or(cursor.point)
        } else {
            cursor.point
        };
    }
    pub fn update(&mut self, text: &str, selected: Option<Range<usize>>) {
        if text.is_empty() {
            self.clear();
            return;
        }
        if self.anchor.is_none() {
            self.anchor = Some(self.resting);
        }
        self.text = text.replace(['\r', '\n'], "");
        let length = self.text.encode_utf16().count();
        let selected = selected.unwrap_or(length..length);
        self.selected = selected.start.min(length)..selected.end.min(length);
    }
    pub fn bounds(
        &self,
        snapshot: &Snapshot,
        origin: Point<Pixels>,
        metrics: CellGeometry,
    ) -> Bounds<Pixels> {
        let anchor = self.anchor.unwrap_or(self.resting);
        let row = (anchor.line as i64 + snapshot.display_offset() as i64)
            .clamp(0, snapshot.grid().screen_lines as i64 - 1) as f32;
        let column = anchor.column.min(snapshot.grid().columns - 1) as f32;
        Bounds {
            origin: point(
                origin.x + px(column * metrics.cell_width),
                origin.y + px(row * metrics.cell_height),
            ),
            size: size(px(metrics.cell_width), px(metrics.cell_height)),
        }
    }
}

/// One unwrapped, underlined overlay prepared before the element paints.
#[derive(Clone)]
pub(super) struct PreeditPaint {
    pub bounds: Bounds<Pixels>,
    pub line: ShapedLine,
    pub palette: TerminalPalette,
}

impl PreeditPaint {
    pub fn new(
        text: String,
        bounds: Bounds<Pixels>,
        fonts: &FontSet,
        metrics: CellGeometry,
        palette: TerminalPalette,
        window: &Window,
    ) -> Self {
        let run = TextRun {
            len: text.len(),
            font: fonts.regular().clone(),
            color: hsla(palette.foreground),
            background_color: None,
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(hsla(palette.foreground)),
                wavy: false,
            }),
            strikethrough: None,
        };
        let line = window.text_system().shape_line(
            text.clone().into(),
            px(metrics.font_size),
            &[run],
            None,
        );
        Self {
            bounds,
            line,
            palette,
        }
    }
    pub fn paint(&self, window: &mut Window, cx: &mut App) {
        let bounds = Bounds {
            origin: self.bounds.origin,
            size: size(
                self.line.width().max(self.bounds.size.width),
                self.bounds.size.height,
            ),
        };
        window.paint_quad(fill(bounds, hsla(self.palette.background)));
        let _ = self.line.paint(
            bounds.origin,
            bounds.size.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}
