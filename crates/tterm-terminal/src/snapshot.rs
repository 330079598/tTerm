//! One immutable frame of the visible area.
//!
//! A [`Snapshot`] is copied out of the terminal while its lock is held and
//! read afterwards, so painting never blocks the parser and never sees a
//! half-applied write (design note 1.3). It contains exactly the visible rows
//! — never the scrollback — plus the metadata a frame needs: cursor, modes,
//! display offset, selection, and which rows changed since the previous frame.

use crate::cell::Cell;
use crate::damage::Damage;
use crate::grid::{CellSize, GridPoint, GridSize, WindowSize};
use crate::mode::Modes;
use crate::selection::SelectionRange;

use alacritty_terminal::vte::ansi::CursorShape as AnsiCursorShape;

/// The shape the cursor is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorShape {
    /// A filled block over the cell.
    #[default]
    Block,
    /// A line under the cell.
    Underline,
    /// A vertical bar at the left edge of the cell.
    Beam,
    /// An outline around the cell.
    HollowBlock,
    /// The application hid the cursor.
    Hidden,
}

impl From<AnsiCursorShape> for CursorShape {
    fn from(shape: AnsiCursorShape) -> Self {
        match shape {
            AnsiCursorShape::Block => Self::Block,
            AnsiCursorShape::Underline => Self::Underline,
            AnsiCursorShape::Beam => Self::Beam,
            AnsiCursorShape::HollowBlock => Self::HollowBlock,
            AnsiCursorShape::Hidden => Self::Hidden,
        }
    }
}

/// Where the cursor is and how to draw it.
///
/// The point is in grid coordinates, as the design note requires; use
/// [`Snapshot::screen_row`] to place it in the frame. When the viewport is
/// scrolled back the cursor can be below it, in which case `screen_row`
/// returns `None` and the cursor is not drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// The cursor position in grid coordinates.
    pub point: GridPoint,
    /// The shape to draw.
    pub shape: CursorShape,
    /// Whether the cursor blinks, according to the application's setting.
    pub blinking: bool,
    /// The cell under the cursor, so a block cursor can invert the right
    /// glyph without asking the rows again. `None` when the cursor is outside
    /// the frame.
    pub cell: Option<Cell>,
}

impl Cursor {
    /// Whether a cursor should be drawn at all.
    pub fn is_visible(&self) -> bool {
        self.shape != CursorShape::Hidden
    }
}

/// One row of the visible grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    cells: Vec<Cell>,
}

impl Row {
    pub(crate) fn new(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    /// The cells of the row, one per column, left to right.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cell at `column`, if the column is inside the row.
    pub fn cell(&self, column: usize) -> Option<&Cell> {
        self.cells.get(column)
    }

    /// The number of cells in the row, which is the grid's column count.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the row has no cells; only possible for a zero-width grid,
    /// which [`GridSize`] does not allow.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// The pieces a snapshot is assembled from.
///
/// This exists so the constructor does not take a dozen positional arguments;
/// only [`crate::terminal::Terminal::snapshot`] builds it.
pub(crate) struct SnapshotParts {
    pub version: u64,
    pub grid: GridSize,
    pub cell_size: CellSize,
    pub rows: Vec<Row>,
    pub cursor: Cursor,
    pub modes: Modes,
    pub display_offset: usize,
    pub history_size: usize,
    pub scrollback_generation: u64,
    pub evicted_lines: u64,
    pub selection: Option<SelectionRange>,
    pub damage: Damage,
    pub bottom_line_occupied: bool,
}

/// The visible area at one point in time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    version: u64,
    grid: GridSize,
    cell_size: CellSize,
    rows: Vec<Row>,
    cursor: Cursor,
    modes: Modes,
    display_offset: usize,
    history_size: usize,
    scrollback_generation: u64,
    evicted_lines: u64,
    selection: Option<SelectionRange>,
    damage: Damage,
    bottom_line_occupied: bool,
}

impl Snapshot {
    pub(crate) fn new(parts: SnapshotParts) -> Self {
        let SnapshotParts {
            version,
            grid,
            cell_size,
            rows,
            cursor,
            modes,
            display_offset,
            history_size,
            scrollback_generation,
            evicted_lines,
            selection,
            damage,
            bottom_line_occupied,
        } = parts;
        Self {
            version,
            grid,
            cell_size,
            rows,
            cursor,
            modes,
            display_offset,
            history_size,
            scrollback_generation,
            evicted_lines,
            selection,
            damage,
            bottom_line_occupied,
        }
    }

    /// A number that increases with every snapshot, so a view can tell two
    /// frames apart. Taking a snapshot consumes and resets the damage, which
    /// is why this is also the point at which it changes (design note 4.1).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The size of the grid this frame was taken from.
    pub fn grid(&self) -> GridSize {
        self.grid
    }

    /// The cell size the view last reported.
    pub fn cell_size(&self) -> CellSize {
        self.cell_size
    }

    /// The visible rows, top to bottom.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The row at `line`, where zero is the top of the viewport.
    pub fn row(&self, line: usize) -> Option<&Row> {
        self.rows.get(line)
    }

    /// The number of visible rows.
    pub fn line_count(&self) -> usize {
        self.rows.len()
    }

    /// The cursor and how to draw it.
    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    /// The terminal modes in effect.
    pub fn modes(&self) -> Modes {
        self.modes
    }

    /// How far the viewport is scrolled back; zero means it shows the newest
    /// output.
    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    /// Whether the viewport is scrolled back into the history.
    pub fn is_scrolled_back(&self) -> bool {
        self.display_offset > 0
    }

    /// Whether the viewport shows the oldest line the terminal still holds.
    pub fn is_scrolled_to_top(&self) -> bool {
        self.display_offset >= self.history_size
    }

    /// Whether the viewport shows the newest output.
    pub fn is_scrolled_to_bottom(&self) -> bool {
        self.display_offset == 0
    }

    /// How many scrollback lines the terminal currently holds, which is at
    /// most the configured capacity.
    pub fn history_size(&self) -> usize {
        self.history_size
    }

    /// The monotonically increasing history generation.
    ///
    /// The generation advances whenever output evicts one or more oldest
    /// scrollback lines. Views use it to invalidate absolute-line caches even
    /// when the bounded history length itself stays unchanged.
    pub fn scrollback_generation(&self) -> u64 {
        self.scrollback_generation
    }

    /// The cumulative number of scrollback lines evicted by output.
    pub fn evicted_lines(&self) -> u64 {
        self.evicted_lines
    }

    /// The selection, in grid coordinates, or `None` when nothing is selected.
    pub fn selection(&self) -> Option<SelectionRange> {
        self.selection
    }

    /// The rows that changed since the previous snapshot.
    pub fn damage(&self) -> &Damage {
        &self.damage
    }

    /// Whether the last visible row contains a glyph.
    ///
    /// The view uses it to decide where to put the pixels left over when the
    /// window height is not a whole number of rows (design note 5.1).
    pub fn bottom_line_occupied(&self) -> bool {
        self.bottom_line_occupied
    }

    /// The text area size, as programs get it back from a `CSI 14 t` query.
    pub fn window_size(&self) -> WindowSize {
        WindowSize {
            grid: self.grid,
            cell: self.cell_size,
        }
    }

    /// The screen row of an absolute grid line: `line + display_offset`,
    /// when that falls inside the frame (design note 4.2).
    ///
    /// This is the one place the conversion is written down. Selection
    /// endpoints, search matches and hyperlinks use absolute lines, drawing
    /// uses screen rows.
    pub fn screen_row(&self, line: i32) -> Option<usize> {
        screen_row(line, self.display_offset, self.rows.len())
    }

    /// The absolute grid line of a visible row.
    ///
    /// This is the inverse of [`Snapshot::screen_row`]: a viewport row plus
    /// the display offset. The hint scanner and any other code that maps a
    /// frame position back to a grid coordinate uses it, so the conversion
    /// stays in one place.
    pub fn grid_line(&self, row: usize) -> Option<i32> {
        if row >= self.rows.len() {
            return None;
        }
        let offset = i32::try_from(self.display_offset).ok()?;
        let line = i32::try_from(row).ok()?;
        line.checked_sub(offset)
    }

    /// The number of cells this frame copied.
    ///
    /// Always `line_count() * grid().columns`: the scrollback is never part of
    /// it, no matter how many lines it holds.
    pub fn cell_count(&self) -> usize {
        self.rows.iter().map(Row::len).sum()
    }
}

/// The screen row of an absolute grid line, when it falls inside a frame with
/// `line_count` rows scrolled back by `display_offset`.
///
/// This is the single implementation of the conversion the design note 4.2
/// describes as `screen row = absolute line + display offset`; the snapshot
/// and the cursor placement both call it.
pub(crate) fn screen_row(line: i32, display_offset: usize, line_count: usize) -> Option<usize> {
    let offset = i32::try_from(display_offset).ok()?;
    let row = line.checked_add(offset)?;
    let row = usize::try_from(row).ok()?;
    (row < line_count).then_some(row)
}
