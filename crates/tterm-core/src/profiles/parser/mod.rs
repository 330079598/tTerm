//! Reading `~/.ssh/config`: `paths` finds the files, `words` splits them into
//! words, `matching` checks `Match` criteria and `pattern` decides what is a
//! wildcard host.

pub mod matching;
pub mod paths;
pub mod pattern;
pub mod words;
