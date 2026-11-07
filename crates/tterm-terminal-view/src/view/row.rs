//! Turning one snapshot row into a cached, paint-ready layout.
//!
//! A row is planned first (what text, backgrounds, decorations and built-in
//! glyphs it contains) and shaped second (one shaping call per merged text
//! run). The result is a flat list of placed glyphs plus spans, which the
//! paint phase can draw without asking the text system anything.
//!
//! Planning and shaping are separate so the grouping rules can be tested
//! without a window, and so a cache hit can skip shaping entirely.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use gpui_kit::{FontId, GlyphId, Pixels, SharedString, WindowTextSystem, px};
use tterm_terminal::cell::{Cell, Underline};
use tterm_terminal::palette::{Rgba, TerminalPalette};

use crate::glyphs::{BuiltinGlyph, CellMetrics, builtin_glyph};

use super::font::FontSet;
use super::style::{ResolvedStyle, TextStyle, hsla};

/// One merged run of text, ready to be shaped.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRunSpec {
    /// The text of the run, including combining marks.
    pub text: String,
    /// For each appended character, its byte offset in `text` and the grid
    /// column it belongs to.
    pub bytes_to_columns: Vec<(usize, usize)>,
    /// The resolved style shared by every cell in the run.
    pub style: TextStyle,
}

/// A background rectangle covering adjacent cells of one color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackgroundSpan {
    /// The first column.
    pub start: usize,
    /// The number of columns.
    pub len: usize,
    /// The color to paint.
    pub color: Rgba,
}

/// An underline covering adjacent cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnderlineSpan {
    /// The first column.
    pub start: usize,
    /// The number of columns.
    pub len: usize,
    /// The decoration.
    pub kind: Underline,
    /// The line color.
    pub color: Rgba,
}

/// A strikethrough covering adjacent cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrikeSpan {
    /// The first column.
    pub start: usize,
    /// The number of columns.
    pub len: usize,
    /// The line color.
    pub color: Rgba,
}

/// A built-in glyph and where it sits on the row.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinCellPlan {
    /// The column the glyph starts at.
    pub column: usize,
    /// The geometry, in device pixels.
    pub glyph: BuiltinGlyph,
    /// The foreground at the time the row was planned.
    pub color: Rgba,
}

/// Everything a row needs before its text is shaped.
#[derive(Clone, Debug, PartialEq)]
pub struct RowPlan {
    /// The number of columns in the row.
    pub width: usize,
    /// The merged text runs.
    pub texts: Vec<TextRunSpec>,
    /// The merged background rectangles.
    pub backgrounds: Vec<BackgroundSpan>,
    /// The built-in glyphs, which bypass font shaping.
    pub builtins: Vec<BuiltinCellPlan>,
    /// The merged underlines.
    pub underlines: Vec<UnderlineSpan>,
    /// The merged strikethroughs.
    pub strikes: Vec<StrikeSpan>,
}

/// Plans one row: merges text styles, backgrounds and decorations, and takes
/// box drawing, blocks and Powerline out of the text path.
pub fn plan_row(cells: &[Cell], palette: &TerminalPalette, metrics: CellMetrics) -> RowPlan {
    let mut plan = RowPlan {
        width: cells.len(),
        texts: Vec::new(),
        backgrounds: Vec::new(),
        builtins: Vec::new(),
        underlines: Vec::new(),
        strikes: Vec::new(),
    };
    let mut pending: Option<TextRunSpec> = None;

    for (column, cell) in cells.iter().enumerate() {
        let resolved = ResolvedStyle::for_cell(cell, palette);
        if resolved.has_background(palette) {
            push_background(&mut plan.backgrounds, column, resolved.background);
        }
        if resolved.text.underline != Underline::None {
            push_underline(
                &mut plan.underlines,
                column,
                resolved.text.underline,
                resolved.text.underline_color,
            );
        }
        if resolved.text.strikeout {
            push_strike(&mut plan.strikes, column, resolved.text.foreground);
        }

        if resolved.text.hidden
            || cell.is_blank()
            || cell.attributes.wide_spacer
            || cell.attributes.leading_wide_spacer
        {
            flush(&mut pending, &mut plan.texts);
            continue;
        }

        if let Some(glyph) = builtin_glyph(cell.ch, metrics) {
            flush(&mut pending, &mut plan.texts);
            plan.builtins.push(BuiltinCellPlan {
                column,
                glyph,
                color: resolved.text.foreground,
            });
            continue;
        }

        if pending
            .as_ref()
            .is_none_or(|run| run.style != resolved.text)
        {
            flush(&mut pending, &mut plan.texts);
            pending = Some(TextRunSpec {
                text: String::new(),
                bytes_to_columns: Vec::new(),
                style: resolved.text,
            });
        }
        if let Some(run) = pending.as_mut() {
            push_char(run, cell.ch, column);
            if let Some(zerowidth) = &cell.zerowidth {
                for character in zerowidth {
                    push_char(run, *character, column);
                }
            }
        }
        // A wide character occupies the next column through its spacer; end
        // the run here so the spacer never becomes a character of its own.
        if cell.attributes.wide {
            flush(&mut pending, &mut plan.texts);
        }
    }
    flush(&mut pending, &mut plan.texts);
    plan
}

fn push_char(run: &mut TextRunSpec, character: char, column: usize) {
    run.bytes_to_columns.push((run.text.len(), column));
    run.text.push(character);
}

fn flush(pending: &mut Option<TextRunSpec>, texts: &mut Vec<TextRunSpec>) {
    if let Some(run) = pending.take()
        && !run.text.is_empty()
    {
        texts.push(run);
    }
}

fn push_background(backgrounds: &mut Vec<BackgroundSpan>, column: usize, color: Rgba) {
    if let Some(last) = backgrounds.last_mut()
        && last.start + last.len == column
        && last.color == color
    {
        last.len += 1;
        return;
    }
    backgrounds.push(BackgroundSpan {
        start: column,
        len: 1,
        color,
    });
}

fn push_underline(
    underlines: &mut Vec<UnderlineSpan>,
    column: usize,
    kind: Underline,
    color: Rgba,
) {
    if let Some(last) = underlines.last_mut()
        && last.start + last.len == column
        && last.kind == kind
        && last.color == color
    {
        last.len += 1;
        return;
    }
    underlines.push(UnderlineSpan {
        start: column,
        len: 1,
        kind,
        color,
    });
}

fn push_strike(strikes: &mut Vec<StrikeSpan>, column: usize, color: Rgba) {
    if let Some(last) = strikes.last_mut()
        && last.start + last.len == column
        && last.color == color
    {
        last.len += 1;
        return;
    }
    strikes.push(StrikeSpan {
        start: column,
        len: 1,
        color,
    });
}

/// One glyph, already positioned relative to its cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    /// The column the glyph belongs to.
    pub column: usize,
    /// The glyph's x offset from the cell's left edge, for clusters that
    /// advance inside one cell.
    pub offset_x: Pixels,
    /// The font the glyph was shaped with.
    pub font_id: FontId,
    /// The glyph in that font.
    pub glyph_id: GlyphId,
    /// The font size the glyph was shaped at.
    pub font_size: Pixels,
    /// The color to draw it with.
    pub color: Rgba,
    /// Whether it is a color emoji glyph.
    pub is_emoji: bool,
}

/// A row ready to paint.
#[derive(Clone, Debug, PartialEq)]
pub struct RowLayout {
    /// The number of columns in the row.
    pub width: usize,
    /// All glyphs, left to right.
    pub glyphs: Vec<PlacedGlyph>,
    /// Merged background rectangles.
    pub backgrounds: Vec<BackgroundSpan>,
    /// Built-in glyphs and their colors.
    pub builtins: Vec<BuiltinCellPlan>,
    /// Merged underlines.
    pub underlines: Vec<UnderlineSpan>,
    /// Merged strikethroughs.
    pub strikes: Vec<StrikeSpan>,
}

impl RowLayout {
    /// An empty row, for cache tests.
    pub fn empty(width: usize) -> Self {
        Self {
            width,
            glyphs: Vec::new(),
            backgrounds: Vec::new(),
            builtins: Vec::new(),
            underlines: Vec::new(),
            strikes: Vec::new(),
        }
    }
}

/// Shapes every text run in `plan` once and flattens the result.
pub fn shape_row(
    plan: RowPlan,
    text_system: &WindowTextSystem,
    fonts: &FontSet,
    font_size: f32,
) -> RowLayout {
    let mut glyphs = Vec::new();
    for spec in &plan.texts {
        let font = fonts.get(spec.style.bold, spec.style.italic).clone();
        let run = gpui_kit::TextRun {
            len: spec.text.len(),
            font,
            color: hsla(spec.style.foreground),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(
            SharedString::from(spec.text.clone()),
            px(font_size),
            &[run],
            None,
        );
        let mut first_x: HashMap<usize, Pixels> = HashMap::new();
        for shaped_run in &shaped.runs {
            for glyph in &shaped_run.glyphs {
                let column = column_for_offset(&spec.bytes_to_columns, glyph.index);
                let first = *first_x.entry(column).or_insert(glyph.position.x);
                glyphs.push(PlacedGlyph {
                    column,
                    offset_x: glyph.position.x - first,
                    font_id: shaped_run.font_id,
                    glyph_id: glyph.id,
                    font_size: px(font_size),
                    color: spec.style.foreground,
                    is_emoji: glyph.is_emoji,
                });
            }
        }
    }
    RowLayout {
        width: plan.width,
        glyphs,
        backgrounds: plan.backgrounds,
        builtins: plan.builtins,
        underlines: plan.underlines,
        strikes: plan.strikes,
    }
}

fn column_for_offset(map: &[(usize, usize)], offset: usize) -> usize {
    match map.binary_search_by_key(&offset, |(byte, _)| *byte) {
        Ok(index) => map[index].1,
        Err(0) => map[0].1,
        Err(index) => map[index - 1].1,
    }
}

/// A content hash of one row plus the style epoch.
///
/// The snapshot version alone cannot be the key: it changes on every
/// snapshot, while the content of most rows stays the same when the screen
/// scrolls. The version is used only as a fast path when a draw reuses the
/// previous frame; invalidation here is by content and by the epoch, which
/// changes when the palette, font or cell metrics change.
pub fn row_hash(cells: &[Cell], style_epoch: u64) -> u64 {
    let mut hasher = FnvHasher::new();
    style_epoch.hash(&mut hasher);
    cells.len().hash(&mut hasher);
    for cell in cells {
        (cell.ch as u32).hash(&mut hasher);
        hash_color(&mut hasher, cell.foreground);
        hash_color(&mut hasher, cell.background);
        cell.attributes.hash(&mut hasher);
        cell.underline_color.hash(&mut hasher);
        cell.zerowidth.hash(&mut hasher);
        cell.hyperlink.is_some().hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_color(hasher: &mut impl Hasher, color: tterm_terminal::color::Color) {
    use tterm_terminal::color::Color;
    match color {
        Color::Named(named) => {
            0u8.hash(hasher);
            (named as u8).hash(hasher);
        }
        Color::Indexed(index) => {
            1u8.hash(hasher);
            index.hash(hasher);
        }
        Color::Spec(rgb) => {
            2u8.hash(hasher);
            rgb.r.hash(hasher);
            rgb.g.hash(hasher);
            rgb.b.hash(hasher);
        }
    }
}

struct FnvHasher(u64);

impl FnvHasher {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_terminal::cell::Attributes;
    use tterm_terminal::color::{Color, NamedColor};

    fn cell(character: char) -> Cell {
        Cell {
            ch: character,
            foreground: Color::Named(NamedColor::Foreground),
            background: Color::Named(NamedColor::Background),
            attributes: Attributes::default(),
            underline_color: None,
            hyperlink: None,
            zerowidth: None,
        }
    }

    fn row(text: &str) -> Vec<Cell> {
        text.chars().map(cell).collect()
    }

    #[test]
    fn same_styles_merge_into_one_run() {
        let cells = row("hello world");
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 2);
        assert_eq!(plan.texts[0].text, "hello");
        assert_eq!(plan.texts[1].text, "world");
    }

    #[test]
    fn a_style_change_breaks_the_run() {
        let mut cells = row("ab");
        cells[1].attributes.bold = true;
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 2);
        assert!(plan.texts[1].style.bold);
    }

    #[test]
    fn adjacent_same_backgrounds_merge() {
        let mut cells = row("abc");
        cells[0].background = Color::Indexed(1);
        cells[1].background = Color::Indexed(1);
        cells[2].background = Color::Indexed(2);
        let palette = TerminalPalette::default();
        let plan = plan_row(&cells, &palette, CellMetrics::new(8.0, 16.0));
        assert_eq!(plan.backgrounds.len(), 2);
        assert_eq!(plan.backgrounds[0].start, 0);
        assert_eq!(plan.backgrounds[0].len, 2);
        assert_eq!(plan.backgrounds[0].color, palette.red);
        assert_eq!(plan.backgrounds[1].len, 1);
    }

    #[test]
    fn adjacent_underlines_merge_and_keep_their_kind() {
        let mut cells = row("abc");
        for cell in &mut cells {
            cell.attributes.underline = Underline::Curly;
        }
        cells[2].attributes.underline = Underline::Double;
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.underlines.len(), 2);
        assert_eq!(plan.underlines[0].len, 2);
        assert_eq!(plan.underlines[0].kind, Underline::Curly);
        assert_eq!(plan.underlines[1].kind, Underline::Double);
    }

    #[test]
    fn wide_characters_take_two_columns_and_their_spacer_is_not_text() {
        let mut wide = cell('中');
        wide.attributes.wide = true;
        let mut spacer = cell(' ');
        spacer.attributes.wide_spacer = true;
        let cells = vec![wide, spacer, cell('a')];
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 2);
        assert_eq!(plan.texts[0].text, "中");
        assert_eq!(plan.texts[0].bytes_to_columns, [(0, 0)]);
        assert_eq!(plan.texts[1].text, "a");
        assert_eq!(plan.texts[1].bytes_to_columns, [(0, 2)]);
    }

    #[test]
    fn combining_marks_stay_in_their_cell() {
        let mut base = cell('e');
        base.zerowidth = Some(vec!['\u{301}']);
        let cells = vec![base, cell('x')];
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 1);
        assert_eq!(plan.texts[0].text, "e\u{301}x");
        assert_eq!(plan.texts[0].bytes_to_columns, [(0, 0), (1, 0), (3, 1)]);
    }

    #[test]
    fn built_in_glyphs_bypass_text() {
        let cells = row("a─b");
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 2);
        assert_eq!(plan.builtins.len(), 1);
        assert_eq!(plan.builtins[0].column, 1);
    }

    #[test]
    fn hidden_cells_keep_their_background_but_not_their_glyph() {
        let mut cells = row("ab");
        cells[0].attributes.hidden = true;
        cells[0].background = Color::Indexed(4);
        let plan = plan_row(
            &cells,
            &TerminalPalette::default(),
            CellMetrics::new(8.0, 16.0),
        );
        assert_eq!(plan.texts.len(), 1);
        assert_eq!(plan.texts[0].text, "b");
        assert_eq!(plan.backgrounds.len(), 1);
    }

    #[test]
    fn row_hashes_follow_content_and_epoch() {
        let cells = row("hello");
        let same = row("hello");
        let changed = row("hellp");
        assert_eq!(row_hash(&cells, 1), row_hash(&same, 1));
        assert_ne!(row_hash(&cells, 1), row_hash(&changed, 1));
        assert_ne!(row_hash(&cells, 1), row_hash(&cells, 2));
    }
}
