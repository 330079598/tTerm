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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use alacritty_terminal::event::Event as AnsiEvent;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Scroll as AnsiScroll;
use alacritty_terminal::grid::{Dimensions, GridIterator};
use alacritty_terminal::index::Point as AnsiPoint;
use alacritty_terminal::selection::Selection as AnsiSelection;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Cell as AnsiCell;
use alacritty_terminal::term::{Config as TermConfig, Osc52, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::Processor;

use crate::cell::Cell;
use crate::command_marks::overview::CommandMarkState;
use crate::damage::Damage;
use crate::event::{self, Clipboard, Intent};
use crate::grid::{CellSize, GridPoint, GridSize};
use crate::history_release::{HistoryRelease, MarkChanges, ScrollRegion};
use crate::mode::Modes;
use crate::mouse_reporting::MOUSE_REPORTING_OFF;
use crate::osc52_scanner::Osc52Scanner;
use crate::selection::{SelectionKind, SelectionRange, Side, word};
use crate::snapshot::{self, Cursor, CursorShape, Row, Snapshot, SnapshotParts};

mod search_ops;
mod selection_text;

/// How many scrollback lines a new model keeps when the caller has no
/// preference.
const DEFAULT_SCROLLBACK: usize = 10_000;

/// The characters xterm treats as word separators by default.
///
/// The Tauri client leaves `wordSeparator` at xterm's default. Unlike
/// Alacritty's default `semantic_escape_chars`, this set keeps `|`, `:`, `<`
/// and `>` inside words. Double-click selection uses these same boundaries.
pub const WORD_SEPARATORS: &str = " ()[]{}',\"`";

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

/// How much time snapshot requests spent waiting for and holding the model lock.
///
/// The view takes one snapshot per visible frame. The wait includes time a
/// parser batch owns the lock; the hold is only the visible-row copy plus the
/// damage reset, never shaping or painting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotTiming {
    /// Number of snapshots taken since the counters were reset.
    pub snapshots: u64,
    /// Total time spent waiting for the lock.
    pub waited: Duration,
    /// Total time the lock was held while copying the snapshot.
    pub held: Duration,
    /// The longest single lock hold.
    pub held_max: Duration,
}

#[derive(Default)]
struct SnapshotCounters {
    snapshots: AtomicU64,
    waited_nanos: AtomicU64,
    held_nanos: AtomicU64,
    held_max_nanos: AtomicU64,
}

impl SnapshotCounters {
    fn record(&self, waited: Duration, held: Duration) {
        let waited = u64::try_from(waited.as_nanos()).unwrap_or(u64::MAX);
        let held = u64::try_from(held.as_nanos()).unwrap_or(u64::MAX);
        self.snapshots.fetch_add(1, Ordering::Relaxed);
        self.waited_nanos.fetch_add(waited, Ordering::Relaxed);
        self.held_nanos.fetch_add(held, Ordering::Relaxed);
        self.held_max_nanos.fetch_max(held, Ordering::Relaxed);
    }

    fn reset(&self) {
        self.snapshots.store(0, Ordering::Relaxed);
        self.waited_nanos.store(0, Ordering::Relaxed);
        self.held_nanos.store(0, Ordering::Relaxed);
        self.held_max_nanos.store(0, Ordering::Relaxed);
    }

    fn timing(&self) -> SnapshotTiming {
        SnapshotTiming {
            snapshots: self.snapshots.load(Ordering::Relaxed),
            waited: Duration::from_nanos(self.waited_nanos.load(Ordering::Relaxed)),
            held: Duration::from_nanos(self.held_nanos.load(Ordering::Relaxed)),
            held_max: Duration::from_nanos(self.held_max_nanos.load(Ordering::Relaxed)),
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
    /// Characters that end a word for double-click selection.
    pub word_separators: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scrollback: DEFAULT_SCROLLBACK,
            kitty_keyboard: false,
            clipboard: ClipboardPolicy::default(),
            word_separators: WORD_SEPARATORS.to_string(),
        }
    }
}

impl Config {
    fn to_alacritty(&self) -> TermConfig {
        TermConfig {
            scrolling_history: self.scrollback,
            kitty_keyboard: self.kitty_keyboard,
            semantic_escape_chars: self.word_separators.clone(),
            // Alacritty is the only clipboard reader. The model's bounded
            // scanner produces write intents so every OSC 52 store has one
            // bounded path while queries still use Alacritty's formatter.
            osc52: match self.clipboard {
                ClipboardPolicy::Disabled | ClipboardPolicy::CopyOnly => Osc52::Disabled,
                ClipboardPolicy::PasteOnly | ClipboardPolicy::CopyAndPaste => Osc52::OnlyPaste,
            },
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
    osc52_scanner: Osc52Scanner,
    command_marks: CommandMarkState,
    config: Config,
    cell_size: CellSize,
    version: u64,
    scrollback_generation: u64,
    content_version: u64,
    scroll_region: ScrollRegion,
    word_selection: bool,
    word_drag: Option<SelectionRange>,
}

/// The terminal of one session.
///
/// One session has one model, which may outlive the view that draws it (a tab
/// moved, a split rearranged, a window rebuilt). The model never holds a PTY
/// or SSH writer; everything it needs written back arrives as an intent.
pub struct Terminal {
    state: FairMutex<State>,
    intents: Arc<Mutex<Vec<Intent>>>,
    snapshot_counters: SnapshotCounters,
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
        let term = Term::new(config.to_alacritty(), &grid.clamped(), collector);
        let osc52_scanner = Osc52Scanner::new(matches!(
            config.clipboard,
            ClipboardPolicy::CopyOnly | ClipboardPolicy::CopyAndPaste
        ));
        let state = State {
            term,
            parser: Processor::new(),
            osc52_scanner,
            command_marks: CommandMarkState::new(),
            config,
            cell_size,
            version: 0,
            scrollback_generation: 0,
            content_version: 0,
            scroll_region: ScrollRegion::full(grid.clamped().screen_lines),
            word_selection: false,
            word_drag: None,
        };
        Self {
            state: FairMutex::new(state),
            intents,
            snapshot_counters: SnapshotCounters::default(),
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
        // A non-empty batch may change any cell, including repainting one
        // visible row in place. Bumping here, inside the lock, makes the
        // version a conservative description of the text a search copies.
        state.content_version = state.content_version.wrapping_add(1);
        // An active drag endpoint is a screen-coordinate choice. Output can
        // scroll or reflow the grid under it, so keep the lifecycle-tracked
        // anchor and drop the endpoint instead of committing a stale range.
        state.word_drag = None;
        let configured = state.config.scrollback;
        let was_alternate = state.term.mode().contains(TermMode::ALT_SCREEN);
        let history_before = if was_alternate {
            0
        } else {
            state
                .term
                .total_lines()
                .saturating_sub(state.term.screen_lines())
        };
        // The scanners only locate complete control strings. A mark is
        // applied after the parser has consumed the bytes up to its end, so
        // its position is the model state after that OSC, not a guessed line.
        let stores = state.osc52_scanner.scan(bytes);
        let mark_events = state.command_marks.scan(bytes);
        let mut events = stores
            .into_iter()
            .map(|(end, text)| (end, Some(text), None))
            .chain(
                mark_events
                    .into_iter()
                    .map(|event| (event.end, None, Some(event))),
            )
            .collect::<Vec<_>>();
        events.sort_by_key(|(end, _, _)| *end);
        let State {
            parser,
            term,
            command_marks,
            scroll_region,
            ..
        } = &mut *state;
        let mut region = *scroll_region;
        let mut cleared_main = false;
        let mut start = 0;
        for (end, store, mark) in events {
            if end < start || end > bytes.len() {
                continue;
            }
            let (changes, cleared) = {
                let mut handler = HistoryRelease::with_region(term, region);
                parser.advance(&mut handler, &bytes[start..end]);
                region = handler.region();
                (handler.mark_changes(), handler.cleared_main())
            };
            cleared_main |= cleared;
            apply_mark_changes(command_marks, changes);
            if let Some(text) = store {
                self.intents
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(Intent::ClipboardStore {
                        clipboard: Clipboard::Clipboard,
                        text,
                    });
            }
            if let Some(event) = mark {
                if parser.sync_bytes_count() > 0 {
                    // The synchronized update has not executed this mark yet.
                    // Its future cursor cannot be known without replaying the
                    // buffered bytes, so drop the marks instead of inventing a
                    // position; the next prompt starts a fresh entry.
                    command_marks.dispose_markers();
                } else {
                    command_marks.handle_event(event, term);
                }
            }
            start = end;
        }
        {
            let mut handler = HistoryRelease::with_region(term, region);
            parser.advance(&mut handler, &bytes[start..]);
            region = handler.region();
            cleared_main |= handler.cleared_main();
            apply_mark_changes(command_marks, handler.mark_changes());
        }
        *scroll_region = region;
        let is_alternate = term.mode().contains(TermMode::ALT_SCREEN);
        if was_alternate != is_alternate {
            // A switch swaps grids within one write. A main clear and every
            // main scroll before the switch were already applied at their
            // exact byte offsets; a new generation drops cached rows.
            state.scrollback_generation = state.scrollback_generation.wrapping_add(1);
            return;
        }
        if is_alternate {
            return;
        }
        let history_size = term.total_lines().saturating_sub(term.screen_lines());
        let full_damage = matches!(term.damage(), TermDamage::Full);
        // A history drop without a clear is still worth invalidating cached
        // absolute lines for.
        let cleared_history = cleared_main || history_before > history_size;
        if cleared_history || (configured > 0 && history_size >= configured && full_damage) {
            state.scrollback_generation = state.scrollback_generation.wrapping_add(1);
        }
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
    ///
    /// The size is clamped like [`Terminal::new`] does, so a one-column or
    /// zero-row size built from the public fields cannot reach Alacritty.
    pub fn resize(&self, grid: GridSize, cell_size: CellSize) {
        let mut state = self.state.lock();
        state.word_drag = None;
        state.cell_size = cell_size;
        // Reflow changes which row holds each line; the tracker has no reflow
        // map, so stale marks are dropped instead of pointing at another row.
        state.command_marks.dispose_markers();
        // Only a grid change can reflow text under a search result. A cell
        // size change alone leaves every retained row where it was.
        let grid = grid.clamped();
        let current = GridSize::new(state.term.columns(), state.term.screen_lines());
        if grid != current {
            state.content_version = state.content_version.wrapping_add(1);
        }
        state.term.resize(grid);
        state.scroll_region = ScrollRegion::full(state.term.screen_lines());
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
        let was_alternate = state.term.mode().contains(TermMode::ALT_SCREEN);
        let history_before = state
            .term
            .total_lines()
            .saturating_sub(state.term.screen_lines());
        // The alternate grid hides the primary history that set_options trims.
        let invalidates =
            lines < state.config.scrollback && (was_alternate || lines < state.term.history_size());
        if invalidates {
            state.scrollback_generation = state.scrollback_generation.wrapping_add(1);
            // The same shrink drops rows from the text a search can see.
            state.content_version = state.content_version.wrapping_add(1);
        }
        state.word_drag = None;
        state.config.scrollback = lines;
        let config = state.config.to_alacritty();
        state.term.set_options(config);
        let history_after = state
            .term
            .total_lines()
            .saturating_sub(state.term.screen_lines());
        if history_after < history_before {
            state
                .command_marks
                .trim_scrollback(history_before - history_after);
        } else if invalidates {
            // The hidden primary screen shrank while the alternate screen was
            // active, so no visible history size can tell how far to move.
            state.command_marks.dispose_markers();
        }
    }

    /// The configured scrollback capacity, in lines.
    pub fn scrollback(&self) -> usize {
        self.state.lock().config.scrollback
    }

    /// The current modes, without copying or consuming the grid's damage.
    pub fn modes(&self) -> Modes {
        Modes::from(*self.state.lock().term.mode())
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
        let requested = Instant::now();
        let mut state = self.state.lock();
        let acquired = Instant::now();
        let version = state.version.wrapping_add(1);
        state.version = version;
        let snapshot = take_snapshot(&mut state, version);
        let held = acquired.elapsed();
        drop(state);
        self.snapshot_counters.record(acquired - requested, held);
        snapshot
    }

    /// Snapshot lock timing since the counters were last reset.
    pub fn snapshot_timing(&self) -> SnapshotTiming {
        self.snapshot_counters.timing()
    }

    /// Clears the snapshot timing counters, which a benchmark does before a
    /// measured run.
    pub fn reset_snapshot_timing(&self) {
        self.snapshot_counters.reset();
    }

    /// Starts a selection at a grid point.
    ///
    /// A click passes [`SelectionKind::Simple`]; double-click passes
    /// [`SelectionKind::Semantic`] and triple-click [`SelectionKind::Lines`].
    /// The pointer's position inside the cell decides the [`Side`].
    pub fn select(&self, kind: SelectionKind, point: GridPoint, side: Side) {
        let mut state = self.state.lock();
        if kind == SelectionKind::Semantic {
            let separators = state.config.word_separators.clone();
            if let Some(range) = word::word_range(&state.term, point, &separators) {
                state.word_selection = true;
                state.word_drag = None;
                word::set_range(&mut state.term, range);
                return;
            }
        }
        state.word_selection = false;
        state.word_drag = None;
        state.term.selection = Some(AnsiSelection::new(kind.into(), point.into(), side.into()));
    }

    /// Moves the endpoint of the current selection.
    ///
    /// Does nothing when there is no selection, so a drag that starts outside
    /// the grid cannot create one.
    pub fn extend_selection(&self, point: GridPoint, side: Side) {
        let mut state = self.state.lock();
        if state.term.selection.is_none() {
            return;
        }
        if state.word_selection {
            let separators = state.config.word_separators.clone();
            if let Some(word) = word::word_range(&state.term, point, &separators) {
                // The anchor selection stays untouched, so Alacritty rotates it
                // through output, resize and eviction. Every drag update rebuilds
                // from that fixed anchor and the word under the pointer, which
                // lets the range shrink and reverse instead of only growing.
                state.word_drag = Some(word);
            }
            return;
        }
        if let Some(selection) = state.term.selection.as_mut() {
            selection.update(point.into(), side.into());
        }
    }

    /// Drops the selection.
    pub fn clear_selection(&self) {
        let mut state = self.state.lock();
        state.term.selection = None;
        state.word_selection = false;
        state.word_drag = None;
    }

    /// Commits an active word drag to the ordinary selection.
    ///
    /// A word drag keeps the original word as its lifecycle-tracked anchor.
    /// Once the pointer is released, the combined range becomes the selection
    /// itself, so later output moves it without needing drag state.
    pub fn finish_selection(&self) {
        let mut state = self.state.lock();
        let Some(drag) = state.word_drag.take() else {
            return;
        };
        let anchor = state
            .term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&state.term))
            .map(SelectionRange::from);
        if let Some(anchor) = anchor {
            word::set_range(
                &mut state.term,
                SelectionRange {
                    start: anchor.start.min(drag.start),
                    end: anchor.end.max(drag.end),
                    block: false,
                },
            );
        }
        state.word_selection = false;
    }

    /// Whether a selection exists, even an empty one that a press just made.
    pub fn has_selection(&self) -> bool {
        self.state.lock().term.selection.is_some()
    }

    /// Whether a program asked for mouse reports (1000, 1002 or 1003).
    pub fn is_mouse_reporting(&self) -> bool {
        self.state
            .lock()
            .term
            .mode()
            .intersects(TermMode::MOUSE_MODE)
    }

    /// Turns mouse reporting off when it is still on, as the Tauri guard does
    /// after a prompt mark or once a local shell is back in the foreground.
    ///
    /// Returns whether the reset was written.
    pub fn clear_mouse_reporting(&self) -> bool {
        if !self.is_mouse_reporting() {
            return false;
        }
        self.write(MOUSE_REPORTING_OFF);
        true
    }
}

/// Applies the history changes the parser made during one slice.
fn apply_mark_changes(command_marks: &mut CommandMarkState, changes: MarkChanges) {
    if changes.dispose {
        command_marks.dispose_markers();
    } else if changes.shift > 0 {
        command_marks.trim_scrollback(changes.shift);
    }
}

/// Builds the frame while the caller holds the lock.
fn take_snapshot(state: &mut State, version: u64) -> Snapshot {
    let selection = selection_text::active_selection_range(state);
    let State {
        term,
        cell_size,
        scrollback_generation,
        command_marks,
        ..
    } = state;
    let command_marks = command_marks.positions();
    let damage = Damage::take(term);
    let grid = GridSize::new(term.columns(), term.screen_lines());
    let history_size = term.total_lines().saturating_sub(grid.screen_lines);
    let content = term.renderable_content();
    let display_offset = content.display_offset;
    let modes = Modes::from(content.mode);
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
        scrollback_generation: *scrollback_generation,
        selection,
        damage,
        bottom_line_occupied,
        command_marks,
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
