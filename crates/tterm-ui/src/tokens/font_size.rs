/// One step of the interface type scale, as `applyUiScalePercent` writes it to
/// the stylesheet: a rem multiplier and the line height in use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSize {
    /// The size in rem, before the interface scale multiplies it.
    pub rem: f32,
    /// The line height as a multiple of the font size.
    pub line_height: f32,
}
