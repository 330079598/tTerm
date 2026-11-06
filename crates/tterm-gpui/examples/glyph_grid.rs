//! P0-28's built-in glyph grid.
//!
//! Paints every character `tterm-terminal-view`'s built-in font covers, in an
//! 18 by 13 grid of 26 px cells. It is deliberately not a terminal element:
//! there is no parser, no grid and no font shaping, only the glyph geometry
//! and GPUI's low-level quad and path painting. Run it with
//!
//! ```sh
//! cargo run -p tterm-gpui --example glyph_grid
//! ```
//!
//! and compare the window with `scripts/parity/p0-28-glyphs.sh` run in the
//! Tauri build.

use std::path::Path;

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, HeadlessAppContext, PathBuilder, Pixels, Point, Rgba, Window, WindowBounds,
    WindowOptions, canvas, fill, point, px, rgb, size,
};
use tterm_terminal_view::glyphs::BuiltinGlyph;
use tterm_terminal_view::glyphs::CellMetrics;
use tterm_terminal_view::glyphs::GlyphFill;
use tterm_terminal_view::glyphs::GlyphShape;
use tterm_terminal_view::glyphs::builtin_glyph;

/// Columns in the grid; the character list fills 18 by 13 exactly.
const COLUMNS: usize = 18;
/// The size of one square cell in device-independent pixels.
const CELL: f32 = 26.0;

/// The characters P0-28 covers, in the order Alacritty's font tests them.
fn characters() -> Vec<char> {
    let mut characters = Vec::new();
    characters.extend('\u{2500}'..='\u{257f}');
    characters.extend('\u{2580}'..='\u{259f}');
    characters.extend('\u{1fb00}'..='\u{1fb3b}');
    characters.extend('\u{1fb82}'..='\u{1fb8b}');
    characters.extend('\u{e0b0}'..='\u{e0b3}');
    characters
}

struct GlyphGrid;

impl Render for GlyphGrid {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        canvas(
            |_bounds, _window, _cx| (),
            |bounds, _, window, _cx| paint(bounds, window),
        )
        .size_full()
    }
}

/// Paints the grid centered in the window.
fn paint(bounds: Bounds<Pixels>, window: &mut Window) {
    window.paint_quad(fill(bounds, rgb(0x0d1117)));
    let characters = characters();
    let rows = characters.len().div_ceil(COLUMNS);
    let grid_width = COLUMNS as f32 * CELL;
    let grid_height = rows as f32 * CELL;
    let origin = point(
        bounds.origin.x + px((f32::from(bounds.size.width) - grid_width) / 2.0),
        bounds.origin.y + px((f32::from(bounds.size.height) - grid_height) / 2.0),
    );

    // The cell backgrounds first, so a glyph painted in one cell is never
    // covered by the next cell's background.
    for index in 0..characters.len() {
        let cell = cell_origin(origin, index);
        let background = if (index / COLUMNS).is_multiple_of(2) {
            rgb(0x161b22)
        } else {
            rgb(0x11161c)
        };
        window.paint_quad(fill(
            Bounds {
                origin: cell,
                size: size(px(CELL), px(CELL)),
            },
            background,
        ));
    }

    for (index, character) in characters.iter().enumerate() {
        let Some(glyph) = builtin_glyph(*character, CellMetrics::new(CELL, CELL)) else {
            continue;
        };
        // Alacritty's shade steps are coverage, so a shade is the foreground
        // color at value/255 opacity rather than a gray of its own.
        let color = match glyph.fill() {
            GlyphFill::Foreground => foreground(),
            GlyphFill::ForegroundAlpha(coverage) => foreground().alpha(f32::from(coverage) / 255.0),
        };
        paint_glyph(window, cell_origin(origin, index), &glyph, color);
    }
}

/// The grid's foreground color.
fn foreground() -> Rgba {
    rgb(0xe6edf3)
}

/// The top-left corner of one cell in the grid.
fn cell_origin(origin: Point<Pixels>, index: usize) -> Point<Pixels> {
    let column = index % COLUMNS;
    let row = index / COLUMNS;
    point(
        origin.x + px(column as f32 * CELL),
        origin.y + px(row as f32 * CELL),
    )
}

/// Paints one glyph's rectangles and polygons at the cell origin.
fn paint_glyph(window: &mut Window, origin: Point<Pixels>, glyph: &BuiltinGlyph, color: Rgba) {
    for shape in glyph.shapes() {
        match shape {
            GlyphShape::Rect(rect) => {
                window.paint_quad(fill(
                    Bounds {
                        origin: point(origin.x + px(rect.x), origin.y + px(rect.y)),
                        size: size(px(rect.width), px(rect.height)),
                    },
                    color,
                ));
            }
            GlyphShape::Polygon(points) => {
                let Some(first) = points.first() else {
                    continue;
                };
                let mut builder = PathBuilder::fill();
                builder.move_to(point(origin.x + px(first.x), origin.y + px(first.y)));
                for glyph_point in &points[1..] {
                    builder.line_to(point(
                        origin.x + px(glyph_point.x),
                        origin.y + px(glyph_point.y),
                    ));
                }
                builder.close();
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
}

fn main() {
    // With no argument the grid opens in a window for a human to look at.
    // `--screenshot <path>` renders the same view headlessly and writes the
    // PNG, which makes the compare asset reproducible on a build machine
    // without screen-recording permission.
    let mut arguments = std::env::args().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (Some("--screenshot"), Some(path)) => screenshot(Path::new(&path)),
        (None, None) => run_window(),
        _ => {
            eprintln!("usage: glyph_grid [--screenshot <path>]");
            std::process::exit(2);
        }
    }
}

fn run_window() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        open_window(cx);
    });
}

/// Renders the grid through the headless Metal renderer and saves it.
fn screenshot(path: &Path) {
    if let Err(error) = capture(path) {
        eprintln!("glyph_grid: could not capture the grid: {error}");
        std::process::exit(1);
    }
    println!("glyph_grid: wrote {}", path.display());
}

fn capture(path: &Path) -> Result<(), String> {
    let text_system = gpui_kit::platform::current_platform(true).text_system();
    let mut context =
        HeadlessAppContext::with_platform(text_system, std::sync::Arc::new(()), || {
            gpui_kit::platform::current_headless_renderer()
        });
    let window = context
        .open_window(size(px(540.), px(420.)), |_, cx| cx.new(|_| GlyphGrid))
        .map_err(|error| error.to_string())?;
    context.run_until_parked();
    let image = context
        .capture_screenshot(window.into())
        .map_err(|error| error.to_string())?;
    image.save(path).map_err(|error| error.to_string())
}

/// Opens the grid window, reporting a failure instead of panicking.
fn open_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(100.), px(100.)),
            size: size(px(540.), px(420.)),
        })),
        ..WindowOptions::default()
    };
    match gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| GlyphGrid)) {
        Ok((handle, _)) => {
            if handle
                .update(cx, |_, window, _| {
                    window.set_window_title("tTerm built-in glyphs");
                })
                .is_err()
            {
                eprintln!("glyph_grid: the window opened but could not be titled");
            }
        }
        Err(error) => {
            eprintln!("glyph_grid: could not open the window: {error}");
            cx.quit();
        }
    }
}
