//! Compiling a query and matching it against one logical line.
//!
//! The engine is `regress`, the repository's ECMAScript matcher. It is the
//! same engine keyword highlighting uses, so `\w`, `\s`, `\d`, lookaround and
//! the `i` flag behave as they do in the frontend.

use regress::{Flags, Regex};

use crate::keyword_highlight::escape_pattern;
use crate::search::{
    RowStart, SearchCancellation, SearchError, SearchMatch, SearchOptions, SearchPosition,
    SearchQuery, UnitPosition,
};

/// The characters the frontend's search addon treats as non-word boundaries.
///
/// `@xterm/addon-search` 0.16 checks the character before and after a match against
/// this fixed set instead of a regular-expression `\b`. Reusing the set keeps
/// the native option's behavior, including its treatment of non-ASCII letters
/// such as Chinese characters.
const NON_WORD_CHARACTERS: &str = " ~!@#$%^&*()+`-=[]{}|\\;:\"',./<>?";

impl SearchQuery {
    /// Compiles `query` with the options the user chose.
    ///
    /// An empty query is accepted and yields no matches. A non-empty regular
    /// expression that does not compile returns an error; it is never
    /// reinterpreted as literal text.
    pub fn new(query: &str, options: SearchOptions) -> Result<Self, SearchError> {
        if query.is_empty() {
            return Ok(Self {
                options,
                empty: true,
                regex: None,
            });
        }
        let pattern = if options.regex {
            query.to_owned()
        } else {
            escape_pattern(query)
        };
        let flags = Flags {
            icase: !options.case_sensitive,
            ..Flags::default()
        };
        let regex =
            Regex::with_flags(&pattern, flags).map_err(|error| SearchError(error.to_string()))?;
        Ok(Self {
            options,
            empty: false,
            regex: Some(regex),
        })
    }

    /// The options this query was compiled with.
    pub fn options(&self) -> SearchOptions {
        self.options
    }

    /// Whether the query is empty, which always produces an empty result.
    pub fn is_empty(&self) -> bool {
        self.empty
    }

    /// Adds every accepted match in `units` to `matches`.
    ///
    /// Returns `false` when `cancellation` stopped the scan. Empty matches
    /// are skipped because they cover no cell.
    pub(crate) fn matches_in(
        &self,
        units: &[u16],
        positions: &[UnitPosition],
        rows: &[RowStart],
        matches: &mut Vec<SearchMatch>,
        cancellation: &SearchCancellation,
    ) -> bool {
        let Some(regex) = &self.regex else {
            return true;
        };
        for matched in regex.find_from_ucs2(units, 0) {
            if cancellation.is_cancelled() {
                return false;
            }
            let start = matched.range.start;
            let end = matched.range.end;
            // The iterator advances past an empty match itself, so skipping
            // one cannot loop forever.
            if start == end {
                continue;
            }
            if (!self.options.whole_word || is_whole_word(units, start, end))
                && let Some(found) = map_range(positions, rows, start, end)
            {
                matches.push(found);
            }
        }
        true
    }
}

/// Maps a UTF-16 range to the cells it covers.
fn map_range(
    positions: &[UnitPosition],
    rows: &[RowStart],
    start: usize,
    end: usize,
) -> Option<SearchMatch> {
    let first = *positions.get(start)?;
    let last_index = end.checked_sub(1)?;
    let last = *positions.get(last_index)?;
    let first_row = row_at(rows, start)?;
    let last_row = row_at(rows, last_index)?;
    Some(SearchMatch {
        start: SearchPosition {
            line: first_row.line,
            column: first.column(),
        },
        end: SearchPosition {
            line: last_row.line,
            column: last.column() + last.width().saturating_sub(1),
        },
    })
}

/// The physical row that owns `unit`.
fn row_at(rows: &[RowStart], unit: usize) -> Option<RowStart> {
    let index = rows
        .partition_point(|row| row.unit <= unit)
        .checked_sub(1)?;
    rows.get(index).copied()
}

/// Whether the match's neighbors are the frontend's non-word characters.
fn is_whole_word(units: &[u16], start: usize, end: usize) -> bool {
    let before = start == 0 || is_non_word(units[start - 1]);
    let after = end == units.len() || is_non_word(units[end]);
    before && after
}

fn is_non_word(unit: u16) -> bool {
    u8::try_from(unit).is_ok_and(|byte| NON_WORD_CHARACTERS.as_bytes().contains(&byte))
}
