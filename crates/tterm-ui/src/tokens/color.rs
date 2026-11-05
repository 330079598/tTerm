/// A colour as the stylesheet states it: the three components of the CSS
/// `hsl()` syntax, with saturation and lightness in percent.
///
/// The tokens are opaque. A rule that draws over a token with an alpha passes
/// the alpha when it builds a GPUI `Hsla`, so it is not part of the token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    /// The angle around the colour wheel, from 0 to 360.
    pub hue: f32,
    /// How far the colour is from grey, from 0 to 100.
    pub saturation: f32,
    /// How light the colour is, from 0 to 100.
    pub lightness: f32,
}

impl Hsl {
    /// Builds a colour from the components the stylesheet writes.
    pub const fn new(hue: f32, saturation: f32, lightness: f32) -> Self {
        Self {
            hue,
            saturation,
            lightness,
        }
    }
}
