//! The paint phase: rectangles and glyphs only, no shaping and no model lock.

use gpui_kit::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, px, size};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::selection::SelectionRange;
use tterm_terminal::snapshot::CursorShape;

use crate::glyphs::{GlyphFill, GlyphShape};

use super::font::CellGeometry;
use super::row::{PlacedGlyph, RowLayout, UnderlineSpan};
use super::style::hsla;

/// Where the cursor is drawn, after focus and blink are applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPaint {
    /// The screen row.
    pub row: usize,
    /// The first column.
    pub column: usize,
    /// How many columns the cursor covers; two for a wide character.
    pub width_cells: usize,
    /// The shape the program asked for.
    pub shape: CursorShape,
    /// Whether the terminal has focus.
    pub focused: bool,
}

impl CursorPaint {
    /// Whether this cursor is a filled block that hides the glyph beneath it.
    pub fn is_filled_block(&self) -> bool {
        self.focused && self.shape == CursorShape::Block
    }
}

/// Everything the paint phase reads for one frame.
pub struct PaintContext<'a> {
    /// The window being drawn into.
    pub window: &'a mut Window,
    /// The top-left corner of the grid.
    pub origin: Point<Pixels>,
    /// The current cell measurements.
    pub metrics: CellGeometry,
    /// The theme colors.
    pub palette: &'a TerminalPalette,
    /// The selection, in absolute grid coordinates.
    pub selection: Option<SelectionRange>,
    /// The snapshot's display offset.
    pub display_offset: usize,
    /// Whether the cursor is in its visible blink phase.
    pub show_cursor: bool,
    /// The cursor, when one is drawn.
    pub cursor: Option<CursorPaint>,
}

impl PaintContext<'_> {
    /// Paints the terminal background.
    pub fn paint_terminal_background(&mut self, bounds: Bounds<Pixels>) {
        self.window
            .paint_quad(fill(bounds, hsla(self.palette.background)));
    }

    /// Paints one row's backgrounds, selection, text and decorations.
    pub fn paint_row(&mut self, row_index: usize, row: &RowLayout) {
        let origin = self.row_origin(row_index);
        let cursor_column = self
            .cursor
            .filter(|cursor| {
                cursor.row == row_index && cursor.is_filled_block() && self.show_cursor
            })
            .map(|cursor| cursor.column);

        for background in &row.backgrounds {
            self.window.paint_quad(fill(
                Bounds {
                    origin: point(
                        origin.x + px(background.start as f32 * self.metrics.cell_width),
                        origin.y,
                    ),
                    size: size(
                        px(background.len as f32 * self.metrics.cell_width),
                        px(self.metrics.cell_height),
                    ),
                },
                hsla(background.color),
            ));
        }

        for (start, len) in
            selection_spans(row_index, row.width, self.selection, self.display_offset)
        {
            self.window.paint_quad(fill(
                Bounds {
                    origin: point(
                        origin.x + px(start as f32 * self.metrics.cell_width),
                        origin.y,
                    ),
                    size: size(
                        px(len as f32 * self.metrics.cell_width),
                        px(self.metrics.cell_height),
                    ),
                },
                hsla(self.palette.selection_background),
            ));
        }

        let selection_color = self.palette.selection_foreground;
        for glyph in &row.glyphs {
            if Some(glyph.column) == cursor_column {
                continue;
            }
            let selected =
                selection_contains(self.selection, row_index, glyph.column, self.display_offset);
            let color = if selected {
                selection_color.unwrap_or(glyph.color)
            } else {
                glyph.color
            };
            self.paint_glyph(origin, glyph, color);
        }

        for builtin in &row.builtins {
            if Some(builtin.column) == cursor_column {
                continue;
            }
            let selected = selection_contains(
                self.selection,
                row_index,
                builtin.column,
                self.display_offset,
            );
            let color = if selected {
                selection_color.unwrap_or(builtin.color)
            } else {
                builtin.color
            };
            self.paint_builtin(origin, builtin.column, &builtin.glyph, color);
        }

        for underline in &row.underlines {
            self.paint_underline(origin, underline);
        }
        for strike in &row.strikes {
            self.window.paint_quad(fill(
                Bounds {
                    origin: point(
                        origin.x + px(strike.start as f32 * self.metrics.cell_width),
                        origin.y + px(self.metrics.cell_height * 0.55),
                    ),
                    size: size(
                        px(strike.len as f32 * self.metrics.cell_width),
                        px(thickness(self.metrics.cell_height)),
                    ),
                },
                hsla(strike.color),
            ));
        }
    }

    /// Paints the cursor over the finished rows.
    pub fn paint_cursor(&mut self, row: &RowLayout) {
        let Some(cursor) = self.cursor else {
            return;
        };
        if !self.show_cursor {
            return;
        }
        let bounds = cursor_bounds(self.origin, &cursor, &self.metrics);
        let origin = bounds.origin;
        let width = bounds.size.width.as_f32();
        let height = bounds.size.height.as_f32();
        let color = hsla(self.palette.cursor);
        let faded = color.opacity(0.55);

        match cursor.shape {
            CursorShape::Hidden => {}
            CursorShape::Block if cursor.focused => {
                self.window.paint_quad(fill(bounds, color));
                let accent = self
                    .palette
                    .cursor_accent
                    .unwrap_or(self.palette.background);
                self.paint_cell_content(cursor.row, row, cursor.column, accent);
            }
            CursorShape::Block => {
                self.paint_outline(bounds, faded);
            }
            CursorShape::Underline => {
                self.window.paint_quad(fill(
                    Bounds {
                        origin: point(origin.x, origin.y + px(height - 2.0)),
                        size: size(px(width), px(2.0)),
                    },
                    if cursor.focused { color } else { faded },
                ));
            }
            CursorShape::Beam => {
                self.window.paint_quad(fill(
                    Bounds {
                        origin,
                        size: size(px(2.0), px(height)),
                    },
                    if cursor.focused { color } else { faded },
                ));
            }
            CursorShape::HollowBlock => {
                self.paint_outline(bounds, faded);
            }
        }
    }

    fn paint_cell_content(
        &mut self,
        row_index: usize,
        row: &RowLayout,
        column: usize,
        color: tterm_terminal::palette::Rgba,
    ) {
        let row_origin = self.row_origin(row_index);
        for glyph in row.glyphs.iter().filter(|glyph| glyph.column == column) {
            self.paint_glyph(row_origin, glyph, color);
        }
        for builtin in row
            .builtins
            .iter()
            .filter(|builtin| builtin.column == column)
        {
            self.paint_builtin(row_origin, column, &builtin.glyph, color);
        }
    }

    fn paint_outline(&mut self, bounds: Bounds<Pixels>, color: Hsla) {
        let thickness = px(1.5);
        let width = bounds.size.width;
        let height = bounds.size.height;
        for piece in [
            Bounds {
                origin: bounds.origin,
                size: size(width, thickness),
            },
            Bounds {
                origin: point(bounds.origin.x, bounds.origin.y + height - thickness),
                size: size(width, thickness),
            },
            Bounds {
                origin: bounds.origin,
                size: size(thickness, height),
            },
            Bounds {
                origin: point(bounds.origin.x + width - thickness, bounds.origin.y),
                size: size(thickness, height),
            },
        ] {
            self.window.paint_quad(fill(piece, color));
        }
    }

    fn paint_glyph(
        &mut self,
        origin: Point<Pixels>,
        glyph: &PlacedGlyph,
        color: tterm_terminal::palette::Rgba,
    ) {
        let position = point(
            origin.x + px(glyph.column as f32 * self.metrics.cell_width) + glyph.offset_x,
            origin.y + px(self.metrics.baseline),
        );
        if glyph.is_emoji {
            let _ =
                self.window
                    .paint_emoji(position, glyph.font_id, glyph.glyph_id, glyph.font_size);
        } else {
            let _ = self.window.paint_glyph(
                position,
                glyph.font_id,
                glyph.glyph_id,
                glyph.font_size,
                hsla(color),
            );
        }
    }

    fn paint_builtin(
        &mut self,
        origin: Point<Pixels>,
        column: usize,
        glyph: &crate::glyphs::BuiltinGlyph,
        color: tterm_terminal::palette::Rgba,
    ) {
        let cell_origin = point(
            origin.x + px(column as f32 * self.metrics.cell_width),
            origin.y,
        );
        let color = match glyph.fill() {
            GlyphFill::Foreground => hsla(color),
            GlyphFill::ForegroundAlpha(coverage) => {
                hsla(color.with_alpha((u16::from(color.a) * u16::from(coverage) / 255) as u8))
            }
        };
        for shape in glyph.shapes() {
            match shape {
                GlyphShape::Rect(rect) => {
                    self.window.paint_quad(fill(
                        Bounds {
                            origin: point(
                                cell_origin.x + px(rect.x / self.metrics.scale_factor),
                                cell_origin.y + px(rect.y / self.metrics.scale_factor),
                            ),
                            size: size(
                                px(rect.width / self.metrics.scale_factor),
                                px(rect.height / self.metrics.scale_factor),
                            ),
                        },
                        color,
                    ));
                }
                GlyphShape::Polygon(points) => {
                    let Some(first) = points.first() else {
                        continue;
                    };
                    let mut builder = PathBuilder::fill();
                    builder.move_to(point(
                        cell_origin.x + px(first.x / self.metrics.scale_factor),
                        cell_origin.y + px(first.y / self.metrics.scale_factor),
                    ));
                    for glyph_point in &points[1..] {
                        builder.line_to(point(
                            cell_origin.x + px(glyph_point.x / self.metrics.scale_factor),
                            cell_origin.y + px(glyph_point.y / self.metrics.scale_factor),
                        ));
                    }
                    builder.close();
                    if let Ok(path) = builder.build() {
                        self.window.paint_path(path, color);
                    }
                }
            }
        }
    }

    fn paint_underline(&mut self, origin: Point<Pixels>, underline: &UnderlineSpan) {
        use tterm_terminal::cell::Underline;
        let width = underline.len as f32 * self.metrics.cell_width;
        let thickness = thickness(self.metrics.cell_height);
        let x = origin.x + px(underline.start as f32 * self.metrics.cell_width);
        let color = hsla(underline.color);
        match underline.kind {
            Underline::None => {}
            Underline::Single => {
                self.window.paint_quad(fill(
                    Bounds {
                        origin: point(x, origin.y + px(self.metrics.cell_height - thickness - 1.0)),
                        size: size(px(width), px(thickness)),
                    },
                    color,
                ));
            }
            Underline::Double => {
                for offset in [thickness + 1.0, thickness * 2.0 + 3.0] {
                    self.window.paint_quad(fill(
                        Bounds {
                            origin: point(x, origin.y + px(self.metrics.cell_height - offset)),
                            size: size(px(width), px(thickness)),
                        },
                        color,
                    ));
                }
            }
            Underline::Curly => {
                let y = origin.y + px(self.metrics.cell_height - 3.0);
                let mut builder = PathBuilder::stroke(px(thickness));
                let step = px(self.metrics.cell_width / 2.0);
                let amplitude = px(1.5);
                builder.move_to(point(x, y));
                let mut offset = 0.0;
                while offset < width {
                    let next = (offset + 3.0).min(width);
                    builder.line_to(point(x + px(next), y + amplitude));
                    offset = next;
                    if offset >= width {
                        break;
                    }
                    let next = (offset + 3.0).min(width);
                    builder.line_to(point(x + px(next), y - amplitude));
                    offset = next;
                }
                let _ = step;
                if let Ok(path) = builder.build() {
                    self.window.paint_path(path, color);
                }
            }
            Underline::Dotted => {
                let y = origin.y + px(self.metrics.cell_height - 2.0);
                let mut offset = 0.0;
                while offset < width {
                    self.window.paint_quad(fill(
                        Bounds {
                            origin: point(x + px(offset), y),
                            size: size(px(thickness), px(thickness)),
                        },
                        color,
                    ));
                    offset += 4.0;
                }
            }
            Underline::Dashed => {
                let y = origin.y + px(self.metrics.cell_height - 2.0);
                let mut offset = 0.0;
                while offset < width {
                    let length = 4.0_f32.min(width - offset);
                    self.window.paint_quad(fill(
                        Bounds {
                            origin: point(x + px(offset), y),
                            size: size(px(length), px(thickness)),
                        },
                        color,
                    ));
                    offset += 7.0;
                }
            }
        }
    }

    fn row_origin(&self, row: usize) -> Point<Pixels> {
        point(
            self.origin.x,
            self.origin.y + px(row as f32 * self.metrics.cell_height),
        )
    }
}

/// The cells the cursor covers, relative to the grid origin.
///
/// Every cursor shape is drawn inside these bounds, so they carry both the
/// row and the column; a wide character makes them two cells wide.
fn cursor_bounds(
    grid_origin: Point<Pixels>,
    cursor: &CursorPaint,
    metrics: &CellGeometry,
) -> Bounds<Pixels> {
    Bounds {
        origin: point(
            grid_origin.x + px(cursor.column as f32 * metrics.cell_width),
            grid_origin.y + px(cursor.row as f32 * metrics.cell_height),
        ),
        size: size(
            px(cursor.width_cells as f32 * metrics.cell_width),
            px(metrics.cell_height),
        ),
    }
}

fn thickness(cell_height: f32) -> f32 {
    (cell_height / 14.0).max(1.0).round()
}

fn selection_contains(
    selection: Option<SelectionRange>,
    row: usize,
    column: usize,
    display_offset: usize,
) -> bool {
    let Some(selection) = selection else {
        return false;
    };
    let Some(line) = i32::try_from(row)
        .ok()
        .and_then(|row| row.checked_sub(i32::try_from(display_offset).ok()?))
    else {
        return false;
    };
    selection.contains(tterm_terminal::grid::GridPoint::new(line, column))
}

fn selection_spans(
    row: usize,
    width: usize,
    selection: Option<SelectionRange>,
    display_offset: usize,
) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for column in 0..width {
        if selection_contains(selection, row, column, display_offset) {
            if let Some(last) = spans.last_mut()
                && last.0 + last.1 == column
            {
                last.1 += 1;
                continue;
            }
            spans.push((column, 1));
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_terminal::grid::GridPoint;

    #[test]
    fn selection_spans_merge_adjacent_columns() {
        let selection = Some(SelectionRange {
            start: GridPoint::new(0, 2),
            end: GridPoint::new(0, 5),
            block: false,
        });
        assert_eq!(selection_spans(0, 10, selection, 0), [(2, 4)]);
    }

    #[test]
    fn selection_uses_the_display_offset() {
        let selection = Some(SelectionRange {
            start: GridPoint::new(-1, 0),
            end: GridPoint::new(-1, 3),
            block: false,
        });
        assert_eq!(selection_spans(0, 8, selection, 1), [(0, 4)]);
        assert!(selection_spans(1, 8, selection, 1).is_empty());
    }

    #[test]
    fn a_block_selection_keeps_the_same_columns_on_each_line() {
        let selection = Some(SelectionRange {
            start: GridPoint::new(0, 2),
            end: GridPoint::new(2, 4),
            block: true,
        });
        assert_eq!(selection_spans(0, 8, selection, 0), [(2, 3)]);
        assert_eq!(selection_spans(1, 8, selection, 0), [(2, 3)]);
        assert_eq!(selection_spans(3, 8, selection, 0), []);
    }

    #[test]
    fn the_cursor_is_a_filled_block_only_when_focused() {
        let cursor = CursorPaint {
            row: 0,
            column: 1,
            width_cells: 1,
            shape: CursorShape::Block,
            focused: true,
        };
        assert!(cursor.is_filled_block());
        let mut unfocused = cursor;
        unfocused.focused = false;
        assert!(!unfocused.is_filled_block());
    }

    #[test]
    fn the_cursor_is_drawn_over_its_own_cell() {
        let metrics = CellGeometry {
            cell_width: 8.0,
            cell_height: 19.0,
            font_size: 14.0,
            baseline: 14.0,
            scale_factor: 1.0,
        };
        let cursor = CursorPaint {
            row: 3,
            column: 5,
            width_cells: 2,
            shape: CursorShape::Block,
            focused: true,
        };
        let bounds = cursor_bounds(point(px(10.0), px(20.0)), &cursor, &metrics);
        assert_eq!(
            bounds.origin,
            point(px(10.0 + 5.0 * 8.0), px(20.0 + 3.0 * 19.0))
        );
        assert_eq!(bounds.size, size(px(16.0), px(19.0)));
    }
}
