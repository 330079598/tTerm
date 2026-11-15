//! GPUI window geometry in logical pixels.
//!
//! GPUI's `WindowBounds` and `Bounds<Pixels>` are logical. The persisted
//! `gpui_window_state` value therefore stores these logical values directly;
//! it never goes through the physical-pixel contract of
//! `tterm_core::window_state::state::Bounds`.
//!
//! GPUI's `PlatformDisplay` does not expose a unified scale factor or a
//! physical monitor topology. Mixed-DPI multi-monitor placement is therefore
//! approximate: GPUI reports display rectangles in its own logical space, and
//! this module stays in that same space rather than guessing a scale.

use gpui_kit::{Bounds, Pixels, point, px, size};
use serde::{Deserialize, Serialize};

/// A logical window rectangle, persisted as the four fields GPUI reports.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogicalBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LogicalBounds {
    pub fn from_bounds(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        }
    }

    pub fn to_bounds(self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(self.x), px(self.y)),
            size: size(px(self.width), px(self.height)),
        }
    }

    /// Makes a value safe to persist. Non-finite coordinates become zero and
    /// a negative size becomes zero; this is the single rule used on both
    /// read and write.
    pub fn sanitized(self) -> Self {
        Self {
            x: finite(self.x),
            y: finite(self.y),
            width: finite(self.width).max(0.0),
            height: finite(self.height).max(0.0),
        }
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// A logical monitor area. This is deliberately a GPUI type rather than the
/// physical `Area` used by the Tauri client.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LogicalArea {
    pub fn from_bounds(bounds: Bounds<Pixels>) -> Self {
        let bounds = LogicalBounds::from_bounds(bounds);
        Self {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        }
    }
}

/// How much of the top edge must be on a monitor for the title bar to be
/// grabbable. The same intent as the Tauri client's `GRAB_WIDTH`, in GPUI's
/// logical unit.
const GRAB_WIDTH: f32 = 100.0;

/// Whether `bounds` has its top edge on a monitor with enough width to drag
/// it. Negative monitor origins (a display left of the primary) are valid.
pub fn top_edge_on_screen(bounds: LogicalBounds, monitors: &[LogicalArea]) -> bool {
    let left = bounds.x;
    let right = left + bounds.width;
    let top = bounds.y;
    let needed = GRAB_WIDTH.min(bounds.width).max(1.0);
    monitors.iter().any(|monitor| {
        let monitor_right = monitor.x + monitor.width;
        let monitor_bottom = monitor.y + monitor.height;
        let overlap = right.min(monitor_right) - left.max(monitor.x);
        top >= monitor.y && top < monitor_bottom && overlap >= needed
    })
}

/// `bounds` moved to the middle of `area` and shrunk to fit inside it.
pub fn centered(bounds: LogicalBounds, area: LogicalArea) -> LogicalBounds {
    let width = bounds.width.min(area.width);
    let height = bounds.height.min(area.height);
    LogicalBounds {
        x: area.x + (area.width - width) / 2.0,
        y: area.y + (area.height - height) / 2.0,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::{LogicalArea, LogicalBounds, centered, top_edge_on_screen};

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> LogicalBounds {
        LogicalBounds {
            x,
            y,
            width,
            height,
        }
    }

    fn area(x: f32, y: f32, width: f32, height: f32) -> LogicalArea {
        LogicalArea {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn logical_geometry_keeps_gpui_units_without_a_scale_field() {
        // This is a unit-contract test, not a multi-DPI native verification.
        // The persisted value is already the coordinate space GPUI uses;
        // no per-monitor scale is guessed or persisted.
        let saved = bounds(120.0, 80.0, 1200.0, 800.0);
        assert_eq!(saved.width, 1200.0);
        assert_eq!(saved.height, 800.0);
        assert_eq!(saved.to_bounds().size.width.as_f32(), 1200.0);
    }

    #[test]
    fn a_negative_monitor_origin_is_on_screen() {
        let left_monitor = area(-1920.0, 0.0, 1920.0, 1080.0);
        assert!(top_edge_on_screen(
            bounds(-1500.0, 100.0, 800.0, 600.0),
            &[left_monitor]
        ));
    }

    #[test]
    fn a_window_moved_to_an_unplugged_monitor_is_off_screen() {
        let monitor = area(0.0, 0.0, 1920.0, 1080.0);
        assert!(!top_edge_on_screen(
            bounds(-3000.0, 100.0, 800.0, 600.0),
            &[monitor]
        ));
        assert!(!top_edge_on_screen(
            bounds(100.0, -900.0, 800.0, 600.0),
            &[monitor]
        ));
    }

    #[test]
    fn a_narrow_window_only_needs_its_own_width() {
        let monitor = area(0.0, 0.0, 1920.0, 1080.0);
        assert!(top_edge_on_screen(
            bounds(0.0, 0.0, 40.0, 600.0),
            &[monitor]
        ));
        assert!(!top_edge_on_screen(
            bounds(0.0, 0.0, 0.0, 600.0),
            &[monitor]
        ));
    }

    #[test]
    fn centering_keeps_a_negative_monitor_origin() {
        let left_monitor = area(-1920.0, 0.0, 1920.0, 1080.0);
        let centered = centered(bounds(-3000.0, 100.0, 800.0, 600.0), left_monitor);
        assert_eq!(centered.x, -1360.0);
        assert_eq!(centered.y, 240.0);
        assert_eq!(centered.width, 800.0);
        assert_eq!(centered.height, 600.0);
    }

    #[test]
    fn centering_shrinks_to_the_monitor() {
        let small = area(0.0, 0.0, 640.0, 480.0);
        let centered = centered(bounds(5000.0, 5000.0, 1200.0, 800.0), small);
        assert_eq!(centered, bounds(0.0, 0.0, 640.0, 480.0));
    }

    #[test]
    fn non_finite_values_are_zeroed_and_negative_sizes_are_rejected() {
        let saved = bounds(f32::NAN, f32::INFINITY, -10.0, f32::NEG_INFINITY).sanitized();
        assert_eq!(
            saved,
            bounds(0.0, 0.0, 0.0, 0.0),
            "one explicit sanitizing rule applies to every non-finite field"
        );
    }

    #[test]
    fn serialized_logical_bounds_round_trip_without_units_loss() {
        let saved = bounds(-120.5, 48.25, 1000.75, 700.5);
        let json = serde_json::to_string(&saved).expect("serialize logical bounds");
        let restored: LogicalBounds =
            serde_json::from_str(&json).expect("deserialize logical bounds");
        assert_eq!(restored, saved);
        assert!(json.contains("\"width\":1000.75"));
    }
}
