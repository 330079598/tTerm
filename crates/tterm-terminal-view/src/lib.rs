//! The GPUI terminal view: input encoders, built-in glyph geometry and the
//! drawing element.
//!
//! The crate holds the pieces that need GPUI's types or are consumed by the
//! drawing layer:
//!
//! - [`input`] turns GPUI keyboard and mouse events into the bytes a program
//!   expects, and decides the selection kind for a click. The terminal modes
//!   and grid points come from [`tterm_terminal`].
//! - [`glyphs`] describes box drawing, block elements and Powerline glyphs as
//!   cell-local rectangles and polygons, which the view paints without asking
//!   the font for a glyph.
//! - [`view`] measures the cell grid, takes snapshots, plans and caches rows,
//!   and draws them with [`gpui_kit::Element`].
//!
//! URL and path recognition deliberately lives in `tterm-terminal`: it only
//! depends on the grid, so it can be tested without a UI.
//!
//! # Frame shape
//!
//! `TerminalView` is the GPUI entity. It owns the row cache, the blink timer
//! and visibility. `TerminalElement` is the custom element: its prepaint
//! takes one snapshot and shapes the rows that changed; its paint only draws
//! the prepared frame. An idle view with no blinking cursor schedules no
//! frame at all, and a hidden view never asks for one.

pub mod glyphs;
pub mod input;
pub mod view;
