//! Turning wheel deltas into whole lines.
//!
//! Scroll wheels report either pixels or lines, and a trackpad can report a
//! fraction of a line. The remainder is kept between events so a slow scroll
//! eventually moves, and is dropped when the direction changes or the view
//! reaches a boundary, so a half-line in one direction never cancels a
//! full line in the other.

use gpui_kit::ScrollDelta;

/// Keeps the sub-line part of wheel movement between events.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollAccumulator {
    remainder: f32,
}

impl ScrollAccumulator {
    /// Converts one event to whole lines; positive goes up into history.
    pub fn lines(&mut self, delta: ScrollDelta, cell_height: f32) -> i32 {
        if !cell_height.is_finite() || cell_height <= 0.0 {
            return 0;
        }
        let pixels = match delta {
            ScrollDelta::Pixels(point) => point.y.as_f32(),
            ScrollDelta::Lines(point) => point.y * cell_height,
        };
        if !pixels.is_finite() {
            return 0;
        }
        if pixels == 0.0 {
            return 0;
        }
        if self.remainder != 0.0 && self.remainder.signum() != pixels.signum() {
            self.remainder = 0.0;
        }
        self.remainder += pixels;
        let lines = (self.remainder / cell_height).trunc();
        self.remainder -= lines * cell_height;
        if lines == 0.0 { 0 } else { lines as i32 }
    }

    /// The pixels still waiting for the next event.
    #[cfg(test)]
    pub fn remainder(&self) -> f32 {
        self.remainder
    }

    /// Drops the remainder, for a boundary or a mode change.
    pub fn reset(&mut self) {
        self.remainder = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, px};

    #[test]
    fn a_line_event_moves_one_line_up() {
        let mut accumulator = ScrollAccumulator::default();
        assert_eq!(
            accumulator.lines(ScrollDelta::Lines(point(0.0, 1.0)), 20.0),
            1
        );
        assert_eq!(
            accumulator.lines(ScrollDelta::Lines(point(0.0, -1.0)), 20.0),
            -1
        );
    }

    #[test]
    fn pixel_fractions_accumulate() {
        let mut accumulator = ScrollAccumulator::default();
        let delta = || ScrollDelta::Pixels(point(px(0.0), px(8.0)));
        assert_eq!(accumulator.lines(delta(), 20.0), 0);
        assert_eq!(accumulator.lines(delta(), 20.0), 0);
        assert_eq!(accumulator.lines(delta(), 20.0), 1);
        assert!((accumulator.remainder() - 4.0).abs() < 1e-4);
    }

    #[test]
    fn reversing_drops_the_remainder() {
        let mut accumulator = ScrollAccumulator::default();
        accumulator.lines(ScrollDelta::Pixels(point(px(0.0), px(8.0))), 20.0);
        assert_eq!(
            accumulator.lines(ScrollDelta::Pixels(point(px(0.0), px(-8.0))), 20.0),
            0
        );
        assert!(accumulator.remainder() < 0.0);
        accumulator.reset();
        assert_eq!(accumulator.remainder(), 0.0);
    }

    #[test]
    fn a_zero_height_does_not_divide_by_zero() {
        let mut accumulator = ScrollAccumulator::default();
        assert_eq!(
            accumulator.lines(ScrollDelta::Pixels(point(px(0.0), px(8.0))), 0.0),
            0
        );
    }

    #[test]
    fn negative_remainders_round_toward_zero() {
        let mut accumulator = ScrollAccumulator::default();
        assert_eq!(
            accumulator.lines(ScrollDelta::Pixels(point(px(0.0), px(-30.0))), 20.0),
            -1
        );
        assert!((accumulator.remainder() + 10.0).abs() < 1e-4);
    }
}
