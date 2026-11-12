//! Theme definitions, the bundled catalog, imports and custom themes.
//!
//! Color parsing, palette derivation and the persisted string forms live in
//! `tterm-terminal`, because that crate draws terminals and cannot depend on
//! this one. Everything that names presets, reads theme files or normalizes a
//! saved custom theme lives here.

/// The bundled theme library.
pub mod catalog;
/// Reading, normalizing and merging custom themes.
pub mod custom;
/// The six built-in themes.
pub mod definitions;
/// Turning terminal schemes into palettes and interface colors.
pub mod derivation;
/// Reading theme files written by other terminals.
pub mod import;
/// Shared theme value types.
pub mod types;
/// Creating an editable theme from a preset.
pub mod utils;
