//! Stable slots keep the drop calculation independent of animated tab positions.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(200);

/// Samples the tab's CSS cubic-bezier(0.2, 0, 0, 1) at a fraction of elapsed time.
fn tab_easing(progress: f32) -> f32 {
    if progress <= 0. || progress >= 1. {
        return progress.clamp(0., 1.);
    }
    let (mut low, mut high) = (0., 1.);
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        let time = mid * (0.6 + mid * (-1.2 + 1.6 * mid));
        if time < progress {
            low = mid;
        } else {
            high = mid;
        }
    }
    let parameter = (low + high) * 0.5;
    parameter * parameter * (3. - 2. * parameter)
}

struct Entry {
    from: f32,
    to: f32,
    start: Instant,
}

impl Entry {
    fn sample(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start);
        if elapsed >= DURATION {
            return self.to;
        }
        let progress = elapsed.as_secs_f32() / DURATION.as_secs_f32();
        self.from + (self.to - self.from) * tab_easing(progress)
    }
}

#[derive(Default)]
pub(super) struct Motion {
    entries: BTreeMap<usize, Entry>,
}

impl Motion {
    pub(super) fn offset(&mut self, id: usize, target: f32, now: Instant) -> f32 {
        if target == 0. && !self.entries.contains_key(&id) {
            return 0.;
        }
        let entry = self.entries.entry(id).or_insert(Entry {
            from: 0.,
            to: target,
            start: now,
        });
        let value = entry.sample(now);
        if entry.to != target {
            *entry = Entry {
                from: value,
                to: target,
                start: now,
            };
        }
        if value == 0. && target == 0. {
            self.entries.remove(&id);
        }
        value
    }
}

pub(super) struct Placement {
    pub source: usize,
    pub target: usize,
    pub span: f32,
}

impl Placement {
    pub(super) fn displacement(&self, index: usize) -> f32 {
        if self.source < index && index <= self.target {
            -self.span
        } else if self.target <= index && index < self.source {
            self.span
        } else {
            0.
        }
    }
}

pub(super) fn destination(centres: &[f32], source: usize, left: f32, right: f32) -> usize {
    centres[..source].partition_point(|centre| *centre <= left)
        + centres[source + 1..].partition_point(|centre| *centre < right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_matches_the_css_curve_at_known_bezier_parameters() {
        assert_eq!(tab_easing(0.), 0.);
        assert_eq!(tab_easing(1.), 1.);
        for (elapsed, expected) in [(0.1, 0.15625), (0.2, 0.5), (0.45, 0.84375)] {
            assert!((tab_easing(elapsed) - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn wide_tab_can_reach_both_narrow_end_slots() {
        let centres = [20., 148., 276.];
        assert_eq!(destination(&centres, 1, 0., 208.), 0);
        assert_eq!(destination(&centres, 1, 44., 252.), 1);
        assert_eq!(destination(&centres, 1, 88., 296.), 2);
        let placement = Placement {
            source: 1,
            target: 2,
            span: 212.,
        };
        assert_eq!(
            [0, 1, 2].map(|index| placement.displacement(index)),
            [0., 0., -212.]
        );
    }

    #[test]
    fn reversal_starts_from_current_position_and_returns_to_rest() {
        let start = Instant::now();
        let middle = start + Duration::from_millis(100);
        let mut motion = Motion::default();
        assert_eq!(motion.offset(7, 80., start), 0.);
        let value = motion.offset(7, 80., middle);
        assert!(value > 0. && value < 80.);
        assert_eq!(motion.offset(7, 0., middle), value);
        assert_eq!(motion.offset(7, 0., middle + DURATION), 0.);
        assert!(motion.entries.is_empty());
    }
}
