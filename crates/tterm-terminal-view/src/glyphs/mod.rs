//! Built-in glyph geometry for characters that must cover their whole cell.
//!
//! The characters and their measurements come from Alacritty's built-in font
//! (see the source headers of the submodules). Alacritty rasterizes them into
//! a bitmap; this crate emits rectangles and polygons in cell-local pixels so
//! P0-10 can paint them with GPUI at any cell size.
