//! Font configuration and cell measurement.
//!
//! The cell grid is measured once per font, size and scale factor. The
//! measurement follows `src/lib/terminalFont.ts` and what the Tauri version
//! actually runs: xterm.js's `CharSizeService`, its WebGL and canvas
//! renderers (both size cells the same way) and `FitAddon`, so the row and
//! column counts match at the same font and window size:
//!
//! - the character width is the advance of `m`, rounded down to whole device
//!   pixels; the letter spacing, rounded, is added in device pixels;
//! - the character height is the font's ascent plus descent rounded up to
//!   whole device pixels, and the cell height is that times the line height,
//!   rounded down;
//! - the available width loses the overview ruler gutter whenever the
//!   terminal has scrollback, and both sides are whole logical pixels, as
//!   `FitAddon` reads them from CSS;
//! - rows and columns are `floor(available / cell)`, with the model's minimum
//!   of two columns.
//!
//! GPUI has no letter spacing, but the terminal sets each cell's x position
//! itself, so a non-zero `letter_spacing` widens the cell and every column
//! stays aligned; the glyph is painted at the cell's left edge.

use gpui_kit::{Font, FontFallbacks, FontFeatures, FontStyle, FontWeight, Pixels, TextSystem, px};
use tterm_terminal::grid::GridSize as TerminalGridSize;

use crate::glyphs::CellMetrics;

/// The font settings the terminal draws with.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalFont {
    /// The CSS-like family chain, first choice first.
    pub families: Vec<String>,
    /// Font size in logical pixels.
    pub size: f32,
    /// Line height multiplier; 1 is the font's own ascent plus descent.
    pub line_height: f32,
    /// Extra width added to every cell. GPUI cannot space the glyphs, but the
    /// grid positions cells itself, so this keeps column counts equal to
    /// xterm.js when a theme sets letter spacing.
    pub letter_spacing: f32,
}

impl TerminalFont {
    /// The first-launch font of the current platform, matching
    /// `defaultTerminalFontFamily` in `src/lib/terminalFont.ts`.
    pub fn platform_default() -> Self {
        Self {
            families: platform_families(),
            size: 14.0,
            line_height: 1.0,
            letter_spacing: 0.0,
        }
    }

    /// The primary family, or a platform monospace fallback.
    pub fn primary_family(&self) -> &str {
        self.families
            .first()
            .map(String::as_str)
            .unwrap_or("monospace")
    }
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::platform_default()
    }
}

fn platform_families() -> Vec<String> {
    let chain = if cfg!(target_os = "macos") {
        r#""JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", Menlo, Monaco, monospace"#
    } else if cfg!(target_os = "windows") {
        r#""JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "Cascadia Code", Consolas, monospace"#
    } else {
        r#""JetBrains Mono Nerd Font", "JetBrainsMono Nerd Font", "JetBrains Mono", "DejaVu Sans Mono", monospace"#
    };
    parse_family_chain(chain)
}

/// Splits a CSS font chain into family names, dropping quotes and blanks.
pub fn parse_family_chain(chain: &str) -> Vec<String> {
    chain
        .split(',')
        .map(|name| name.trim().trim_matches(['"', '\'']).to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// The four faces the terminal uses.
#[derive(Clone)]
pub struct FontSet {
    regular: Font,
    bold: Font,
    italic: Font,
    bold_italic: Font,
}

impl FontSet {
    /// Builds the faces with the chain's first family as the primary face,
    /// without asking a text system which families exist.
    ///
    /// The view starts from this set and replaces it with
    /// [`FontSet::resolve`] before it measures the first frame.
    pub fn new(font: &TerminalFont) -> Self {
        Self::with_primary(font, font.primary_family())
    }

    /// Builds the faces from the first family in the chain that
    /// `text_system` has, as a browser picks from a CSS `font-family` list.
    ///
    /// GPUI only looks at a font's fallbacks for characters the primary face
    /// lacks; a primary family that is not installed silently becomes the
    /// platform UI font, which is proportional. Resolving the chain here
    /// keeps the terminal on the same monospace font as xterm.js. When no
    /// family is installed the first one is kept and GPUI's own fallback
    /// applies.
    pub fn resolve(font: &TerminalFont, text_system: &TextSystem) -> Self {
        let primary = first_available(&font.families, |family| {
            family_installed(text_system, family)
        })
        .unwrap_or_else(|| font.primary_family());
        Self::with_primary(font, primary)
    }

    fn with_primary(font: &TerminalFont, primary: &str) -> Self {
        Self {
            regular: face(font, primary, false, false),
            bold: face(font, primary, true, false),
            italic: face(font, primary, false, true),
            bold_italic: face(font, primary, true, true),
        }
    }

    /// The family the faces are drawn with.
    pub fn family(&self) -> &str {
        &self.regular.family
    }

    /// The face for a cell's weight and slant.
    pub fn get(&self, bold: bool, italic: bool) -> &Font {
        match (bold, italic) {
            (false, false) => &self.regular,
            (true, false) => &self.bold,
            (false, true) => &self.italic,
            (true, true) => &self.bold_italic,
        }
    }

    /// The regular face, for measuring the grid.
    pub fn regular(&self) -> &Font {
        &self.regular
    }
}

/// The first family in `families` for which `installed` is true.
fn first_available(families: &[String], mut installed: impl FnMut(&str) -> bool) -> Option<&str> {
    families
        .iter()
        .map(String::as_str)
        .find(|family| installed(family))
}

/// Whether `family` resolves to itself rather than to GPUI's fallback stack.
///
/// GPUI resolves a missing family to the first font of its fallback stack
/// and records only directly loaded fonts under their own name, so a family
/// is installed exactly when the font recorded for its id has its name.
fn family_installed(text_system: &TextSystem, family: &str) -> bool {
    let candidate = gpui_kit::font(family.to_string());
    let id = text_system.resolve_font(&candidate);
    text_system
        .get_font_for_id(id)
        .is_some_and(|resolved| resolved.family == candidate.family)
}

fn face(font: &TerminalFont, primary: &str, bold: bool, italic: bool) -> Font {
    let mut face = gpui_kit::font(primary.to_string());
    // Terminals must not ligate: each cell is positioned on its own.
    face.features = FontFeatures::disable_ligatures();
    let fallbacks: Vec<String> = font
        .families
        .iter()
        .filter(|family| family.as_str() != primary)
        .cloned()
        .collect();
    if !fallbacks.is_empty() {
        face.fallbacks = Some(FontFallbacks::from_fonts(fallbacks));
    }
    if bold {
        face.weight = FontWeight::BOLD;
    }
    if italic {
        face.style = FontStyle::Italic;
    }
    face
}

/// The measured size of one cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellGeometry {
    /// Cell width in logical pixels.
    pub cell_width: f32,
    /// Cell height in logical pixels.
    pub cell_height: f32,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Baseline offset from the top of the cell, in logical pixels.
    pub baseline: f32,
    /// The window's scale factor when the metrics were measured.
    pub scale_factor: f32,
}

impl CellGeometry {
    /// The cell size in device pixels, which built-in glyph geometry uses.
    pub fn device_metrics(&self) -> CellMetrics {
        CellMetrics::new(
            self.cell_width * self.scale_factor,
            self.cell_height * self.scale_factor,
        )
    }

    /// The cell size rounded to whole device pixels, as the model stores it.
    pub fn model_cell_size(&self) -> tterm_terminal::grid::CellSize {
        tterm_terminal::grid::CellSize::new(
            (self.cell_width * self.scale_factor).round().max(1.0) as u16,
            (self.cell_height * self.scale_factor).round().max(1.0) as u16,
        )
    }
}

/// Measures the grid cell for `font` at `scale_factor`.
pub fn measure_cell(
    text_system: &TextSystem,
    fonts: &FontSet,
    font: &TerminalFont,
    scale_factor: f32,
) -> CellGeometry {
    let font_size = px(font.size.max(1.0));
    let font_id = text_system.resolve_font(fonts.regular());
    let advance = text_system
        .advance(font_id, font_size, 'm')
        .map(|size| size.width)
        .or_else(|_| {
            text_system
                .advance(font_id, font_size, '0')
                .map(|size| size.width)
        })
        .unwrap_or_else(|_| px(font.size.max(1.0) * 0.6));
    let ascent = text_system.ascent(font_id, font_size).as_f32();
    // DirectWrite (the Windows platform text system) reports `descent` as a
    // negative number, while the shared `FontMetrics` contract and the other
    // platforms use the distance below the baseline. Take the magnitude so
    // the cell is as tall as the glyphs on every platform.
    let descent = text_system.descent(font_id, font_size).as_f32().abs();
    let font_height = ascent + descent;
    let (cell_width, cell_height) = cell_size(advance.as_f32(), font_height, font, scale_factor);
    let baseline = (cell_height - font_height) / 2.0 + ascent;

    CellGeometry {
        cell_width,
        cell_height,
        font_size: font_size.as_f32(),
        baseline,
        scale_factor,
    }
}

/// The logical cell size for a character `advance` and `font_height`.
///
/// This is xterm.js's renderer arithmetic: the character width is rounded
/// down and the character height rounded up to whole device pixels, the
/// rounded letter spacing is added in device pixels, and the line height
/// scales the rounded character height before it is rounded down. A 14px
/// JetBrains Mono advance of 8.4 therefore makes 8px cells at every common
/// scale factor, as it does in the Tauri version.
fn cell_size(advance: f32, font_height: f32, font: &TerminalFont, scale_factor: f32) -> (f32, f32) {
    let char_width_device = (advance * scale_factor).floor().max(1.0);
    let cell_width_device = (char_width_device + font.letter_spacing.round()).max(1.0);
    let char_height_device = (font_height * scale_factor).ceil().max(1.0);
    let cell_height_device = (char_height_device * font.line_height.max(0.1))
        .floor()
        .max(1.0);
    (
        cell_width_device / scale_factor,
        cell_height_device / scale_factor,
    )
}

/// The width xterm.js leaves free on the right for tTerm's overview ruler
/// and scrollbar: `overviewRuler.width` in `openTerminal.ts`, which
/// `FitAddon` subtracts whenever the terminal keeps scrollback. P0-12 draws
/// the scrollbar in this gutter.
pub const SCROLLBAR_GUTTER: f32 = 10.0;

/// The number of rows and columns that fit in `width` by `height`.
///
/// `has_scrollback` reserves [`SCROLLBAR_GUTTER`] as `FitAddon` does. Both
/// sides are first cut to whole logical pixels, which is what `FitAddon`
/// gets from `parseInt` on the computed CSS size; the small tolerance keeps a
/// layout result such as 799.9999 from losing a pixel.
pub fn grid_for(
    width: Pixels,
    height: Pixels,
    geometry: &CellGeometry,
    has_scrollback: bool,
) -> TerminalGridSize {
    let whole = |value: f32| (value + 1e-3).floor().max(0.0);
    let gutter = if has_scrollback {
        SCROLLBAR_GUTTER
    } else {
        0.0
    };
    let available_width = (whole(width.as_f32()) - gutter).max(0.0);
    let available_height = whole(height.as_f32());
    let columns = ((available_width / geometry.cell_width) + 1e-4)
        .floor()
        .max(0.0) as usize;
    let rows = ((available_height / geometry.cell_height) + 1e-4)
        .floor()
        .max(0.0) as usize;
    TerminalGridSize::new(columns, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(width: f32, height: f32) -> CellGeometry {
        CellGeometry {
            cell_width: width,
            cell_height: height,
            font_size: 14.0,
            baseline: height * 0.8,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn family_chains_drop_quotes_and_blanks() {
        assert_eq!(
            parse_family_chain(r#""JetBrains Mono", Menlo,  monospace, "#),
            ["JetBrains Mono", "Menlo", "monospace"]
        );
    }

    #[test]
    fn the_first_installed_family_wins() {
        let chain = parse_family_chain(
            r#""JetBrains Mono Nerd Font", "JetBrains Mono", Consolas, monospace"#,
        );
        let installed = ["Consolas", "JetBrains Mono"];
        assert_eq!(
            first_available(&chain, |family| installed.contains(&family)),
            Some("JetBrains Mono")
        );
        assert_eq!(first_available(&chain, |_| false), None);
    }

    fn grid(width: f32, height: f32, cell: &CellGeometry, scrollback: bool) -> (usize, usize) {
        let grid = grid_for(px(width), px(height), cell, scrollback);
        (grid.columns, grid.screen_lines)
    }

    #[test]
    fn grid_counts_reserve_the_gutter_and_keep_the_minimums() {
        // JetBrains Mono 14px in xterm.js: 8px cells and 19px rows at DPR 1.
        let cell = geometry(8.0, 19.0);
        assert_eq!(grid(800.0, 600.0, &cell, true), (98, 31));
        assert_eq!(grid(1200.0, 800.0, &cell, true), (148, 42));
        assert_eq!(grid(800.0, 600.0, &cell, false), (100, 31));
        assert_eq!(grid(1.0, 1.0, &cell, true), (2, 1));
    }

    #[test]
    fn fractional_sizes_are_cut_to_whole_pixels_like_fit_addon() {
        // 19.2px rows are JetBrains Mono 14px at DPR 1.25.
        let cell = geometry(8.0, 19.2);
        // FitAddon reads 38 from 38.5px, which holds one 19.2px row, not two.
        assert_eq!(grid(800.0, 38.5, &cell, true).1, 1);
        // A layout result just under a whole pixel keeps that pixel.
        assert_eq!(grid(809.999_9, 600.0, &cell, true).0, 100);
    }

    #[test]
    fn cells_follow_the_xterm_renderer_rounding() {
        let font = TerminalFont::default();
        // An 8.4px advance and an 18.48px font height (JetBrains Mono 14px).
        assert_eq!(cell_size(8.4, 18.48, &font, 1.0), (8.0, 19.0));
        assert_eq!(cell_size(8.4, 18.48, &font, 1.25), (8.0, 24.0 / 1.25));
        assert_eq!(cell_size(8.4, 18.48, &font, 1.5), (8.0, 28.0 / 1.5));
        assert_eq!(cell_size(8.4, 18.48, &font, 2.0), (8.0, 18.5));

        let spaced = TerminalFont {
            letter_spacing: 1.0,
            line_height: 1.2,
            ..TerminalFont::default()
        };
        // Letter spacing is in device pixels: one extra device pixel at DPR 2.
        assert_eq!(cell_size(8.4, 18.48, &spaced, 2.0), (8.5, 44.0 / 2.0));
    }

    #[test]
    fn the_model_cell_size_rounds_to_device_pixels() {
        let mut cell = geometry(8.4, 18.48);
        cell.scale_factor = 1.25;
        assert_eq!(cell.model_cell_size().width, 11);
        assert_eq!(cell.model_cell_size().height, 23);
        let metrics = cell.device_metrics();
        assert!((metrics.width - 10.5).abs() < 1e-4);
        assert!((metrics.height - 23.1).abs() < 1e-4);
    }

    #[test]
    fn the_default_font_matches_the_frontend() {
        let font = TerminalFont::default();
        assert_eq!(font.size, 14.0);
        assert_eq!(font.line_height, 1.0);
        assert_eq!(font.letter_spacing, 0.0);
        assert!(font.families.iter().any(|family| family == "monospace"));
        assert_eq!(font.families[0], "JetBrains Mono Nerd Font");
    }
}
