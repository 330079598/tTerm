//! The GPUI-facing terminal view pieces that do not draw a terminal.
//!
//! The crate holds the input protocol encoders and the built-in glyph
//! geometry, both of which need GPUI's types or are consumed by the drawing
//! layer. It intentionally has no `Element`, no font measurement, no layout
//! and no paint loop: those belong to P0-10.
//!
//! # Layering
//!
//! - [`input`] turns GPUI keyboard and mouse events into the bytes a program
//!   expects, and decides the selection kind for a click. The terminal modes
//!   and grid points come from [`tterm_terminal`].
//! - [`glyphs`] describes box drawing, block elements and Powerline glyphs as
//!   cell-local rectangles and polygons, which P0-10 paints with GPUI.
//!
//! URL and path recognition deliberately lives in `tterm-terminal`: it only
//! depends on the grid, so it can be tested without a UI.

pub mod glyphs;
pub mod input;
