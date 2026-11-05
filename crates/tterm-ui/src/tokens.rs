//! The design tokens of the interface, generated from `src/styles/tokens.json`.
//!
//! A token keeps the value the stylesheet uses, not a value the drawing layer
//! needs: a colour is three HSL components, and a font size is a rem
//! multiplier with the line height the stylesheet applies to it. Turning them
//! into a GPUI `Hsla` or `Pixels` belongs to the view that draws, which also
//! knows the interface scale and the alpha its rule passes.

pub mod color;
pub mod font_size;
pub mod palette;
pub mod theme;
pub mod type_scale;

#[cfg(test)]
mod tests;
