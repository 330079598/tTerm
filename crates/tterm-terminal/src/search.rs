//! Text search over the grid and the retained scrollback.
//!
//! Two APIs share this module. [`Search`] is the frame-sized matcher P0-09
//! exposes through [`crate::terminal::Terminal::find_next`]: Alacritty's own regular
//! expression engine with its smart-case rule, enough for "find the next
//! match" from the cursor.
//!
//! P2-07 needs the other shape: every match in the retained history, with
//! options the user controls, produced by a worker that does not hold the
//! model lock for the whole scan. [`SearchQuery`] compiles the user's query
//! once; [`crate::terminal::Terminal::search_retained`] asks the model for bounded
//! chunks of [`RetainedRows`] and this module feeds them to the scan below.
//! The scan only sees a row's text and the cell behind each UTF-16 unit, so it
//! can run on any thread.
//!
//! # Coordinates
//!
//! A [`SearchPosition`] uses the retained-history coordinate P2-06's overview
//! marks use: line zero is the oldest row the model still holds, and the last
//! visible line is `history_size + screen_lines - 1`. New output that does not
//! evict a row leaves those numbers alone.
//!
//! A result also carries a content version sampled under the same lock as the
//! rows. It advances on every non-empty model write, on a resize that changes
//! the grid and on a scrollback-capacity change that can drop history, so a
//! worker can tell whether the text it scanned is still current. Unlike
//! [`crate::snapshot::Snapshot::scrollback_generation`], it is not a cache
//! generation: taking a snapshot or scrolling does not touch it. The version
//! is only meaningful within one session; two models may share a number.
//!
//! # Matching
//!
//! `regress` supplies the ECMAScript semantics the frontend's
//! `@xterm/addon-search` uses and this repository already relies on for keyword
//! highlights: `\w`, `\s`, `\d`, lookaround and the `i` flag. Whole-word
//! search uses the frontend's non-word character set rather than a regular
//! expression word boundary, so the option behaves like the Tauri client.
//! A pattern that matches the empty string is valid but produces no result;
//! search results are ranges of at least one cell.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use alacritty_terminal::index::Direction as AnsiDirection;
use alacritty_terminal::index::Side as AnsiSide;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::RegexSearch;

use crate::grid::{GridPoint, GridRange};

mod query;
pub(crate) mod rows;
mod scan;

#[cfg(test)]
mod tests;

/// The most retained rows one lock acquisition copies for a background search.
///
/// A worker copies a chunk, releases the model lock, and scans it. The bound
/// keeps the parser's longest wait proportional to one chunk instead of the
/// whole scrollback.
pub const SEARCH_CHUNK_ROWS: usize = 512;

/// Which way to search from the origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchDirection {
    /// Towards the end of the buffer, as the user types a query.
    Forward,
    /// Towards the start of the buffer, for "find previous".
    Backward,
}

/// A compiled search pattern.
pub struct Search {
    regex: RegexSearch,
}

impl Search {
    /// Compiles `pattern` as a regular expression.
    ///
    /// A pattern without an uppercase character matches case-insensitively,
    /// matching how Alacritty's own search behaves.
    pub fn new(pattern: &str) -> Result<Self, SearchError> {
        RegexSearch::new(pattern)
            .map(|regex| Self { regex })
            .map_err(|error| SearchError(error.to_string()))
    }

    /// Finds the next match from `origin`, in `direction`.
    pub(crate) fn find_in<T>(
        &mut self,
        term: &Term<T>,
        origin: GridPoint,
        direction: SearchDirection,
    ) -> Option<GridRange> {
        let direction = match direction {
            SearchDirection::Forward => AnsiDirection::Right,
            SearchDirection::Backward => AnsiDirection::Left,
        };
        // The match's start point is compared with the origin, which makes a
        // match that begins exactly at the origin count as found.
        let matched = term.search_next(
            &mut self.regex,
            origin.into(),
            direction,
            AnsiSide::Left,
            None,
        )?;
        Some((matched.start().to_owned()..=matched.end().to_owned()).into())
    }
}

/// How the user's query is matched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SearchOptions {
    /// Treat the query as an ECMAScript regular expression instead of text.
    pub regex: bool,
    /// Match case exactly instead of folding it.
    pub case_sensitive: bool,
    /// Require the frontend's non-word characters around the match.
    pub whole_word: bool,
}

/// A query compiled for one or more scans.
///
/// Compilation happens once so a worker does not pay the regular-expression
/// cost for every chunk. An empty query is valid and produces no matches; an
/// invalid pattern is an error even in regular-expression mode, where it is
/// never silently treated as literal text.
#[derive(Clone, Debug)]
pub struct SearchQuery {
    options: SearchOptions,
    empty: bool,
    regex: Option<regress::Regex>,
}

/// A point inside the retained history.
///
/// `line` is the retained-history coordinate: zero is the oldest row the
/// model still holds. It is the same coordinate as
/// [`crate::command_marks::CommandMarkPosition`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchPosition {
    /// The retained-history line.
    pub line: i64,
    /// The zero-based cell column.
    pub column: usize,
}

/// One match, from its first cell to its last cell inclusive.
///
/// A wide character's cell covers both of its columns, so `end.column` is the
/// second column for a match that ends on a wide glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchMatch {
    /// The first cell of the match.
    pub start: SearchPosition,
    /// The last cell of the match, inclusive.
    pub end: SearchPosition,
}

/// Every match found in one generation of the retained history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    generation: u64,
    content_version: u64,
    total_lines: usize,
    matches: Vec<SearchMatch>,
}

impl SearchResult {
    pub(crate) fn new(
        generation: u64,
        content_version: u64,
        total_lines: usize,
        matches: Vec<SearchMatch>,
    ) -> Self {
        Self {
            generation,
            content_version,
            total_lines,
            matches,
        }
    }

    /// The scrollback generation the result belongs to.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The content version the scan started from.
    ///
    /// Compare it with
    /// [`crate::terminal::Terminal::content_version`] before using a result.
    /// The number is only comparable within the same session; another model
    /// may produce the same value.
    pub fn content_version(&self) -> u64 {
        self.content_version
    }

    /// The number of retained rows the scan covered.
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// The matches, in reading order.
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// The number of matches.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Whether the scan found no match.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// The index of the match a next/previous command would select.
    ///
    /// `Forward` selects the first match starting at or after `origin`;
    /// `Backward` selects the last match starting at or before it. A caller
    /// moving from the current match adds or subtracts one itself.
    pub fn index_from(&self, origin: SearchPosition, direction: SearchDirection) -> Option<usize> {
        match direction {
            SearchDirection::Forward => {
                let index = self
                    .matches
                    .partition_point(|matched| matched.start < origin);
                (index < self.matches.len()).then_some(index)
            }
            SearchDirection::Backward => self
                .matches
                .partition_point(|matched| matched.start <= origin)
                .checked_sub(1),
        }
    }
}

/// What a retained-history scan produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchOutcome {
    /// The scan covered every retained row at the result's generation.
    Complete(SearchResult),
    /// The caller asked the scan to stop.
    Cancelled,
    /// The content changed during the scan, so its partial matches were
    /// discarded and the caller should scan again. Continuous output can
    /// keep returning `Stale`; a scan is not guaranteed to finish while the
    /// session never stops writing.
    Stale,
}

/// A one-shot stop flag shared by the caller and the scan.
///
/// Clones refer to the same flag. Once cancelled, the scan stops between rows
/// and between matches and returns [`SearchOutcome::Cancelled`].
#[derive(Clone, Debug, Default)]
pub struct SearchCancellation {
    cancelled: Arc<AtomicBool>,
}

impl SearchCancellation {
    /// Creates a flag that has not been cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops a scan that shares this flag.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Whether the scan has been stopped.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// A bounded copy of consecutive retained rows.
///
/// The model produces one of these per lock acquisition. `generation` is the
/// scrollback generation at the copy, `content_version` is the text version
/// sampled with it, and `first_line` is the retained line of `rows[0]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedRows {
    generation: u64,
    content_version: u64,
    total_lines: usize,
    first_line: usize,
    rows: Vec<SearchRow>,
}

impl RetainedRows {
    pub(crate) fn new(
        generation: u64,
        content_version: u64,
        total_lines: usize,
        first_line: usize,
        rows: Vec<SearchRow>,
    ) -> Self {
        Self {
            generation,
            content_version,
            total_lines,
            first_line,
            rows,
        }
    }

    /// The scrollback generation the copy belongs to.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The content version sampled in the same lock acquisition.
    pub fn content_version(&self) -> u64 {
        self.content_version
    }

    /// The number of retained rows in the buffer at the copy.
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// The retained line of the first row in this copy.
    pub fn first_line(&self) -> usize {
        self.first_line
    }

    /// The copied rows.
    pub fn rows(&self) -> &[SearchRow] {
        &self.rows
    }
}

/// One retained row converted to searchable text.
///
/// `units` is UTF-16 so `regress` applies the same indexing the frontend's
/// JavaScript engine does. Wide-character spacer cells are omitted, and a
/// trailing run of blanks is removed from the final row of a hard line. A
/// wrapped row keeps its full width because it continues on the next row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRow {
    line: i64,
    units: Vec<u16>,
    positions: Vec<UnitPosition>,
    wrapped: bool,
}

impl SearchRow {
    pub(crate) fn new(
        line: i64,
        units: Vec<u16>,
        positions: Vec<UnitPosition>,
        wrapped: bool,
    ) -> Self {
        Self {
            line,
            units,
            positions,
            wrapped,
        }
    }

    /// The retained-history line of this row.
    pub fn line(&self) -> i64 {
        self.line
    }

    /// Whether the next retained row continues this one.
    pub fn is_wrapped(&self) -> bool {
        self.wrapped
    }

    /// The row's text as UTF-16 units, without wide-character spacers.
    pub fn units(&self) -> &[u16] {
        &self.units
    }
}

/// The cell each UTF-16 unit belongs to, packed into one word.
///
/// A logical line can hold a match start and end in different physical rows,
/// so the row's line is kept once per row in [`RowStart`] instead of once per
/// UTF-16 unit. The column fits in 24 bits, which is far beyond any terminal
/// width, and the remaining bits hold a one- or two-column width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitPosition {
    packed: u32,
}

impl UnitPosition {
    const COLUMN_MASK: u32 = 0x00FF_FFFF;
    const WIDTH_SHIFT: u32 = 24;

    /// Packs `column` and the cell's `width`, or `None` for a width too large
    /// to be a terminal column.
    pub(crate) fn new(column: usize, width: usize) -> Option<Self> {
        let column = u32::try_from(column).ok()?;
        if column > Self::COLUMN_MASK {
            return None;
        }
        let width = u32::try_from(width.saturating_sub(1)).ok()?;
        Some(Self {
            packed: column | (width << Self::WIDTH_SHIFT),
        })
    }

    fn column(self) -> usize {
        (self.packed & Self::COLUMN_MASK) as usize
    }

    fn width(self) -> usize {
        ((self.packed >> Self::WIDTH_SHIFT) & 0xFF) as usize + 1
    }
}

/// The first UTF-16 unit of one physical row in a joined logical line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RowStart {
    unit: usize,
    line: i64,
}

/// The pure scan over already-copied rows.
struct SearchScan<'a> {
    query: &'a SearchQuery,
    cancellation: &'a SearchCancellation,
    generation: u64,
    content_version: u64,
    total_lines: usize,
    pending_units: Vec<u16>,
    pending_positions: Vec<UnitPosition>,
    pending_rows: Vec<RowStart>,
    matches: Vec<SearchMatch>,
    cancelled: bool,
}

/// Scans every retained row, fetching bounded chunks with `fetch`.
///
/// The loop never holds a model lock because it only calls `fetch`, which is
/// expected to copy one chunk and return. Every chunk must belong to the same
/// content version and the same total row count as the first one, and no
/// chunk may reach past that total; otherwise the partial result is discarded
/// as [`SearchOutcome::Stale`]. Reading the version again after the last chunk
/// catches a write that happened after the last copy.
pub(crate) fn scan_chunks<F>(
    query: &SearchQuery,
    cancellation: &SearchCancellation,
    mut fetch: F,
) -> SearchOutcome
where
    F: FnMut(usize, usize) -> RetainedRows,
{
    if cancellation.is_cancelled() {
        return SearchOutcome::Cancelled;
    }
    if query.empty {
        return scan_empty(fetch, cancellation);
    }
    let first = fetch(0, SEARCH_CHUNK_ROWS);
    let generation = first.generation;
    let content_version = first.content_version;
    let total_lines = first.total_lines;
    let mut scan = SearchScan::new(
        query,
        cancellation,
        generation,
        content_version,
        total_lines,
    );
    let mut next = 0;
    let mut chunk = first;
    loop {
        if cancellation.is_cancelled() {
            return SearchOutcome::Cancelled;
        }
        if chunk.generation != generation
            || chunk.content_version != content_version
            || chunk.total_lines != total_lines
            || chunk.first_line != next
        {
            return SearchOutcome::Stale;
        }
        let end = next.saturating_add(chunk.rows.len());
        if end > total_lines {
            // A growing buffer must never make the result cover rows the
            // result's own total does not name.
            return SearchOutcome::Stale;
        }
        let previous = next;
        scan.push_rows(&chunk.rows);
        next = end;
        if scan.cancelled {
            return SearchOutcome::Cancelled;
        }
        if next >= total_lines {
            // The last chunk may have raced with a write that kept the row
            // count but changed text, so sample the version once more.
            let last = fetch(total_lines, 0);
            if cancellation.is_cancelled() {
                return SearchOutcome::Cancelled;
            }
            if last.generation != generation
                || last.content_version != content_version
                || last.total_lines != total_lines
                || last.first_line != total_lines
            {
                return SearchOutcome::Stale;
            }
            return scan.finish();
        }
        if next == previous {
            return SearchOutcome::Stale;
        }
        chunk = fetch(next, SEARCH_CHUNK_ROWS);
    }
}

/// The empty-query fast path: no regular expression can match a cell range.
fn scan_empty<F>(mut fetch: F, cancellation: &SearchCancellation) -> SearchOutcome
where
    F: FnMut(usize, usize) -> RetainedRows,
{
    if cancellation.is_cancelled() {
        return SearchOutcome::Cancelled;
    }
    let first = fetch(0, 0);
    if cancellation.is_cancelled() {
        return SearchOutcome::Cancelled;
    }
    let last = fetch(first.total_lines, 0);
    if cancellation.is_cancelled() {
        return SearchOutcome::Cancelled;
    }
    if last.generation != first.generation
        || last.content_version != first.content_version
        || last.total_lines != first.total_lines
        || last.first_line != first.total_lines
    {
        return SearchOutcome::Stale;
    }
    SearchOutcome::Complete(SearchResult::new(
        first.generation,
        first.content_version,
        first.total_lines,
        Vec::new(),
    ))
}

/// Why a search pattern could not be compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchError(String);

impl SearchError {
    /// The reason without the display prefix.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid search pattern: {}", self.0)
    }
}

impl std::error::Error for SearchError {}
