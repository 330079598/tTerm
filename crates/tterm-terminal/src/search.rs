//! Text search over the grid.
//!
//! The matcher is Alacritty's own: the same regular-expression engine it uses
//! for its search bar, including its smart-case rule (a pattern without an
//! uppercase letter is case-insensitive). P0-09 exposes a single "find the
//! next match" call; a cancellable background scan over the whole scrollback
//! belongs to P2-07, which can reuse this type from a worker thread.

use std::fmt;

use alacritty_terminal::index::Direction as AnsiDirection;
use alacritty_terminal::index::Side as AnsiSide;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::RegexSearch;

use crate::grid::{GridPoint, GridRange};

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

/// Why a search pattern could not be compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchError(String);

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid search pattern: {}", self.0)
    }
}

impl std::error::Error for SearchError {}
