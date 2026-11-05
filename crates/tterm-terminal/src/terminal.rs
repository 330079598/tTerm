//! The terminal model: one Alacritty terminal behind one fair mutex.
//!
//! # Threading
//!
//! The parse thread calls [`Terminal::write`] with batches of UTF-8 bytes and
//! [`Terminal::drain_intents`] afterwards; the view thread calls
//! [`Terminal::snapshot`] once per frame. Both take the same
//! [`FairMutex`](alacritty_terminal::sync::FairMutex), so a snapshot request
//! that is already waiting is served after the batch in progress instead of
//! being starved by a parser that immediately re-locks (design note 3.2). The
//! lock is only ever held for parsing a batch or copying the visible cells;
//! layout, shaping, color resolution and drawing happen after it is released.
//!
//! The parser must release the lock between batches. That is the caller's
//! job — it owns the bounded queue in front of [`Terminal::write`] — because
//! this crate cannot know how much CPU time is left in a frame.

use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::Event as AnsiEvent;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Scroll as AnsiScroll;
use alacritty_terminal::grid::{Dimensions, GridIterator};
use alacritty_terminal::index::Point as AnsiPoint;
use alacritty_terminal::selection::Selection as AnsiSelection;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Cell as AnsiCell;
use alacritty_terminal::term::{Config as TermConfig, Osc52, Term};
use alacritty_terminal::vte::ansi::Processor;

use crate::cell::Cell;
use crate::damage::Damage;
use crate::event::{self, Intent};
use crate::grid::{CellSize, GridPoint, GridRange, GridSize};
use crate::mode::Modes;
use crate::search::{Search, SearchDirection};
use crate::selection::{SelectionKind, SelectionRange, Side};
use crate::snapshot::{self, Cursor, CursorShape, Row, Snapshot, SnapshotParts};

/// How many scrollback lines a new model keeps when the caller has no
/// preference.
const DEFAULT_SCROLLBACK: usize = 10_000;

/// What programs may do with the clipboard through OSC 52.
///
/// Reading the clipboard is a paste the user did not ask for, so it is denied
/// by default while storing is allowed; the session layer still decides
/// whether to act on [`Intent::ClipboardStore`]. P2-12 defines the single
/// responder for these queries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ClipboardPolicy {
    /// Ignore OSC 52 in both directions.
    Disabled,
    /// Let programs store text, but not read the clipboard.
    #[default]
    CopyOnly,
    /// Let programs read the clipboard, but not store text.
    PasteOnly,
    /// Allow both directions.
    CopyAndPaste,
}

impl From<ClipboardPolicy> for Osc52 {
    fn from(policy: ClipboardPolicy) -> Self {
        match policy {
            ClipboardPolicy::Disabled => Self::Disabled,
            ClipboardPolicy::CopyOnly => Self::OnlyCopy,
            ClipboardPolicy::PasteOnly => Self::OnlyPaste,
            ClipboardPolicy::CopyAndPaste => Self::CopyPaste,
        }
    }
}

/// How a program asked the viewport to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scroll {
    /// Move by a number of lines; positive values go back into the history.
    Delta(i32),
    /// Move one page back into the history.
    PageUp,
    /// Move one page towards the newest output.
    PageDown,
    /// Jump to the oldest line still held.
    Top,
    /// Jump back to the newest output.
    Bottom,
}

impl From<Scroll> for AnsiScroll {
    fn from(scroll: Scroll) -> Self {
        match scroll {
            Scroll::Delta(lines) => Self::Delta(lines),
            Scroll::PageUp => Self::PageUp,
            Scroll::PageDown => Self::PageDown,
            Scroll::Top => Self::Top,
            Scroll::Bottom => Self::Bottom,
        }
    }
}

/// The settings of one terminal model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// How many scrollback lines to keep. Shrinking it drops the oldest lines.
    pub scrollback: usize,
    /// Whether to speak the Kitty keyboard protocol, which P0-28 encodes.
    pub kitty_keyboard: bool,
    /// What programs may do with the clipboard.
    pub clipboard: ClipboardPolicy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scrollback: DEFAULT_SCROLLBACK,
            kitty_keyboard: false,
            clipboard: ClipboardPolicy::default(),
        }
    }
}

impl Config {
    fn to_alacritty(&self) -> TermConfig {
        TermConfig {
            scrolling_history: self.scrollback,
            kitty_keyboard: self.kitty_keyboard,
            osc52: self.clipboard.into(),
            ..TermConfig::default()
        }
    }
}

/// Receives Alacritty's events and turns them into [`Intent`]s.
///
/// It holds only the intent queue: the model's own state is behind the
/// terminal lock, which the parser already holds when an event arrives.
struct Collector {
    intents: Arc<Mutex<Vec<Intent>>>,
}

impl EventListener for Collector {
    fn send_event(&self, event: AnsiEvent) {
        let Some(intent) = event::from_ansi(event) else {
            return;
        };
        let mut intents = self.intents.lock().unwrap_or_else(PoisonError::into_inner);
        intents.push(intent);
    }
}

/// The state the lock protects: the terminal, its parser, and the metadata
/// the snapshot needs that Alacritty does not store.
struct State {
    term: Term<Collector>,
    parser: Processor,
    config: Config,
    cell_size: CellSize,
    version: u64,
}

/// The terminal of one session.
///
/// One session has one model, which may outlive the view that draws it (a tab
/// moved, a split rearranged, a window rebuilt). The model never holds a PTY
/// or SSH writer; everything it needs written back arrives as an intent.
pub struct Terminal {
    state: FairMutex<State>,
    intents: Arc<Mutex<Vec<Intent>>>,
}

impl Terminal {
    /// Creates a model with an empty grid.
    ///
    /// `grid` and `cell_size` are the view's measurements. The row count is
    /// raised to one and a one-column grid to two, so Alacritty never indexes
    /// past its last column and never reflows a one-column grid (see
    /// [`GridSize::MIN_COLUMNS`]).
    pub fn new(config: Config, grid: GridSize, cell_size: CellSize) -> Self {
        let intents = Arc::new(Mutex::new(Vec::new()));
        let collector = Collector {
            intents: Arc::clone(&intents),
        };
        let term = Term::new(config.to_alacritty(), &grid, collector);
        let state = State {
            term,
            parser: Processor::new(),
            config,
            cell_size,
            version: 0,
        };
        Self {
            state: FairMutex::new(state),
            intents,
        }
    }

    /// Feeds a batch of session output to the parser.
    ///
    /// The bytes are UTF-8: a session in another encoding is converted by core
    /// before it reaches the model. A sequence split across two calls is
    /// continued by the parser, which keeps its own state.
    pub fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut state = self.state.lock();
        let State { term, parser, .. } = &mut *state;
        parser.advance(term, bytes);
    }

    /// Takes every intent produced since the last call, in order.
    ///
    /// The caller is the session's parse loop; it resolves the intents and
    /// writes any resulting bytes back through the session's ordered input
    /// channel.
    pub fn drain_intents(&self) -> Vec<Intent> {
        let mut intents = self.intents.lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut *intents)
    }

    /// Resizes the grid and remembers the cell size the view measured.
    pub fn resize(&self, grid: GridSize, cell_size: CellSize) {
        let mut state = self.state.lock();
        state.cell_size = cell_size;
        state.term.resize(grid);
    }

    /// Changes the scrollback capacity.
    ///
    /// Shrinking drops the oldest history lines, which the caller should treat
    /// as a whole-screen refresh; Alacritty marks the grid fully damaged.
    ///
    /// The change goes through Alacritty's option update, which re-announces
    /// the window title as an intent (`Title` or `TitleReset`), as its own
    /// configuration reload does.
    pub fn set_scrollback(&self, lines: usize) {
        let mut state = self.state.lock();
        if state.config.scrollback == lines {
            return;
        }
        state.config.scrollback = lines;
        let config = state.config.to_alacritty();
        state.term.set_options(config);
    }

    /// The configured scrollback capacity, in lines.
    pub fn scrollback(&self) -> usize {
        self.state.lock().config.scrollback
    }

    /// The current grid size.
    pub fn grid_size(&self) -> GridSize {
        let state = self.state.lock();
        GridSize::new(state.term.columns(), state.term.screen_lines())
    }

    /// Moves the viewport through the history.
    ///
    /// Scrolling changes the display offset and Alacritty marks the whole
    /// screen damaged, because every screen row now shows another absolute
    /// line (design note 4.4).
    pub fn scroll(&self, scroll: Scroll) {
        self.state.lock().term.scroll_display(scroll.into());
    }

    /// Copies the visible area into an immutable frame.
    ///
    /// This is the only method a drawing layer needs to call, and it is the
    /// only place that holds the lock on the view thread. The copy is exactly
    /// the visible cells; the scrollback is not touched.
    pub fn snapshot(&self) -> Snapshot {
        let mut state = self.state.lock();
        let version = state.version.wrapping_add(1);
        state.version = version;
        take_snapshot(&mut state, version)
    }

    /// Starts a selection at a grid point.
    ///
    /// A click passes [`SelectionKind::Simple`]; double-click passes
    /// [`SelectionKind::Semantic`] and triple-click [`SelectionKind::Lines`].
    /// The pointer's position inside the cell decides the [`Side`].
    pub fn select(&self, kind: SelectionKind, point: GridPoint, side: Side) {
        let mut state = self.state.lock();
        state.term.selection = Some(AnsiSelection::new(kind.into(), point.into(), side.into()));
    }

    /// Moves the endpoint of the current selection.
    ///
    /// Does nothing when there is no selection, so a drag that starts outside
    /// the grid cannot create one.
    pub fn extend_selection(&self, point: GridPoint, side: Side) {
        let mut state = self.state.lock();
        if let Some(selection) = state.term.selection.as_mut() {
            selection.update(point.into(), side.into());
        }
    }

    /// Drops the selection.
    pub fn clear_selection(&self) {
        self.state.lock().term.selection = None;
    }

    /// The selected text, with trailing blanks trimmed and wrapped lines
    /// joined.
    ///
    /// The caller should only ask when the user copies: a selection over the
    /// whole scrollback would otherwise build a huge string every frame
    /// (design note 6.3).
    pub fn selection_text(&self) -> Option<String> {
        self.state.lock().term.selection_to_string()
    }

    /// Finds the next match of `search` from `origin`.
    ///
    /// The match is computed while the terminal lock is held, so a scan that
    /// walks far into the scrollback delays the parser. P2-07 moves long
    /// searches to a worker that takes the lock in chunks; this call is the
    /// frame-sized version the first prototype needs.
    pub fn find_next(
        &self,
        search: &mut Search,
        origin: GridPoint,
        direction: SearchDirection,
    ) -> Option<GridRange> {
        let state = self.state.lock();
        search.find_in(&state.term, origin, direction)
    }
}

/// Builds the frame while the caller holds the lock.
fn take_snapshot(state: &mut State, version: u64) -> Snapshot {
    let State {
        term, cell_size, ..
    } = state;
    let damage = Damage::take(term);
    let grid = GridSize::new(term.columns(), term.screen_lines());
    let history_size = term.total_lines().saturating_sub(grid.screen_lines);
    let content = term.renderable_content();
    let display_offset = content.display_offset;
    let modes = Modes::from(content.mode);
    let selection = content.selection.map(SelectionRange::from);
    let shape = CursorShape::from(content.cursor.shape);
    let point = content.cursor.point;
    let blinking = term.cursor_style().blinking;
    let (rows, bottom_line_occupied) = copy_rows(content.display_iter, grid);
    let cursor = build_cursor(point, shape, blinking, &rows, display_offset);
    Snapshot::new(SnapshotParts {
        version,
        grid,
        cell_size: *cell_size,
        rows,
        cursor,
        modes,
        display_offset,
        history_size,
        selection,
        damage,
        bottom_line_occupied,
    })
}

/// Copies exactly the visible cells, row by row.
///
/// The iterator is Alacritty's display iterator, which starts at the top of
/// the viewport: no history line is read even when the scrollback holds
/// hundreds of thousands of them.
fn copy_rows(cells: GridIterator<'_, AnsiCell>, grid: GridSize) -> (Vec<Row>, bool) {
    let mut cells = cells;
    let mut rows = Vec::with_capacity(grid.screen_lines);
    for _ in 0..grid.screen_lines {
        let row: Vec<Cell> = cells
            .by_ref()
            .take(grid.columns)
            .map(|indexed| Cell::from_ansi(indexed.cell))
            .collect();
        rows.push(Row::new(row));
    }
    let bottom_line_occupied = rows
        .last()
        .is_some_and(|row| row.cells().iter().any(|cell| !cell.is_blank()));
    (rows, bottom_line_occupied)
}

/// Places the cursor in the frame and attaches the cell it covers.
fn build_cursor(
    point: AnsiPoint,
    shape: CursorShape,
    blinking: bool,
    rows: &[Row],
    display_offset: usize,
) -> Cursor {
    let point = GridPoint::from(point);
    let cell = snapshot::screen_row(point.line, display_offset, rows.len())
        .and_then(|line| rows.get(line))
        .and_then(|row| {
            // A wide character's cursor sits on the spacer cell; Alacritty
            // moves it back so the block covers the glyph itself.
            let column = match row.cell(point.column) {
                Some(cell) if cell.attributes.wide_spacer => point.column.saturating_sub(1),
                _ => point.column,
            };
            row.cell(column).cloned()
        });
    Cursor {
        point,
        shape,
        blinking,
        cell,
    }
}
