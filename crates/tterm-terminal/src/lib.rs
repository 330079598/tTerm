//! The terminal model: screen and history, selection, search, input encoding
//! and the escape sequences tTerm extends.
//!
//! It depends on neither Tauri nor GPUI; the GPUI element that draws it lives
//! in its own crate, `tterm-terminal-view`. The model is imported from
//! nyaterm's terminal crate in P0-09 and split to fit the repository's size
//! limits in P1-13, so this crate is empty until then.
