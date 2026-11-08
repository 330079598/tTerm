//! Scrollbar geometry shared by the terminal element and pointer handling.

/// The minimum thumb size keeps a large scrollback usable on a short track.
const MIN_THUMB: f32 = 18.0;

/// A vertical scrollbar's track and thumb in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarGeometry {
    /// Track height in logical pixels.
    pub track: f32,
    /// Thumb height in logical pixels.
    pub thumb: f32,
    /// Thumb top in logical pixels from the track origin.
    pub top: f32,
}

impl ScrollbarGeometry {
    /// Computes geometry from history lines, visible lines and display offset.
    pub fn new(track: f32, visible_lines: usize, history_size: usize, offset: usize) -> Self {
        let track = track.max(0.0);
        let visible = visible_lines.max(1) as f32;
        let total = (history_size as f32 + visible).max(visible);
        let thumb = (track * visible / total).clamp(MIN_THUMB.min(track), track);
        let travel = (track - thumb).max(0.0);
        let max_offset = history_size as f32;
        let fraction = if max_offset == 0.0 {
            0.0
        } else {
            (offset as f32 / max_offset).clamp(0.0, 1.0)
        };
        Self {
            track,
            thumb,
            top: travel * (1.0 - fraction),
        }
    }

    /// Converts a pointer position within the track to a display offset.
    pub fn offset_at(&self, pointer_y: f32, history_size: usize) -> usize {
        let travel = (self.track - self.thumb).max(0.0);
        if travel == 0.0 || history_size == 0 {
            return 0;
        }
        let thumb_center = (pointer_y - self.thumb / 2.0).clamp(0.0, travel);
        let fraction_from_bottom = 1.0 - thumb_center / travel;
        (fraction_from_bottom * history_size as f32).round() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_reaches_bottom_and_top_at_the_expected_offsets() {
        let top = ScrollbarGeometry::new(240.0, 24, 240, 240);
        let bottom = ScrollbarGeometry::new(240.0, 24, 240, 0);
        assert_eq!(top.top, 0.0);
        assert!((bottom.top - (bottom.track - bottom.thumb)).abs() < 1e-4);
    }

    #[test]
    fn drag_round_trips_through_offset() {
        let geometry = ScrollbarGeometry::new(300.0, 20, 180, 90);
        let center = geometry.top + geometry.thumb / 2.0;
        assert!((geometry.offset_at(center, 180) as i32 - 90).abs() <= 1);
    }

    #[test]
    fn empty_history_has_no_scroll() {
        let geometry = ScrollbarGeometry::new(300.0, 20, 0, 0);
        assert_eq!(geometry.thumb, 300.0);
        assert_eq!(geometry.offset_at(100.0, 0), 0);
    }
}
