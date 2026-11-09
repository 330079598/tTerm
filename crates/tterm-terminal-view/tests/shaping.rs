//! Real-platform text shaping tests.
//!
//! Cargo runs ordinary test functions on worker threads. macOS AppKit requires
//! the platform (and therefore CoreText's text system) to be created on the
//! process's main thread, so this test binary owns its tiny harness and calls
//! `current_platform` from `main`.

use std::sync::Arc;

use gpui_kit::{TextSystem, WindowTextSystem};
use tterm_terminal::cell::{Attributes, Cell};
use tterm_terminal::color::{Color, NamedColor};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal_view::glyphs::CellMetrics;
use tterm_terminal_view::view::font::{FontSet, TerminalFont};
use tterm_terminal_view::view::row::{plan_row, shape_row};

fn shaping_places_each_glyph_on_its_column() {
    let platform = gpui_kit::platform::current_platform(true);
    let text_system = TextSystem::new(platform.text_system());
    let window_text = WindowTextSystem::new(Arc::new(text_system));
    let font = TerminalFont::default();
    let fonts = FontSet::new(&font);
    let cells: Vec<Cell> = "abc"
        .chars()
        .map(|ch| Cell {
            ch,
            foreground: Color::Named(NamedColor::Foreground),
            background: Color::Named(NamedColor::Background),
            attributes: Attributes::default(),
            underline_color: None,
            hyperlink: None,
            zerowidth: None,
        })
        .collect();
    let plan = plan_row(
        &cells,
        &TerminalPalette::default(),
        CellMetrics::new(8.0, 16.0),
    );
    let layout = shape_row(plan, &window_text, &fonts, 14.0);
    assert_eq!(layout.glyphs.len(), 3);
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.column)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(
        layout
            .glyphs
            .iter()
            .all(|glyph| glyph.offset_x.as_f32().abs() < 0.01)
    );
}

fn main() {
    shaping_places_each_glyph_on_its_column();
}
