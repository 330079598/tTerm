//! Whether saved window bounds still fit the monitors that are connected.

use super::state::Bounds;

/// A monitor's area in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// How much of the window's top edge, in physical pixels, must lie on a
/// monitor for the title bar to be grabbed and the window moved.
const GRAB_WIDTH: i64 = 100;

/// Whether a window at `bounds` has its top edge on some monitor, with
/// enough of it there to drag the window. Bounds saved on a monitor that has
/// since been unplugged, or with the title bar above every screen, fail.
pub fn top_edge_on_screen(bounds: Bounds, monitors: &[Area]) -> bool {
    let left = i64::from(bounds.x);
    let right = left + i64::from(bounds.width);
    let top = i64::from(bounds.y);
    let needed = GRAB_WIDTH.min(i64::from(bounds.width)).max(1);
    monitors.iter().any(|monitor| {
        let monitor_left = i64::from(monitor.x);
        let monitor_right = monitor_left + i64::from(monitor.width);
        let monitor_top = i64::from(monitor.y);
        let monitor_bottom = monitor_top + i64::from(monitor.height);
        let overlap = right.min(monitor_right) - left.max(monitor_left);
        top >= monitor_top && top < monitor_bottom && overlap >= needed
    })
}

/// `bounds` moved to the middle of `area` and shrunk to fit inside it.
pub fn centered(bounds: Bounds, area: Area) -> Bounds {
    let width = bounds.width.min(area.width);
    let height = bounds.height.min(area.height);
    let offset = |free: u32| i32::try_from(free / 2).unwrap_or(i32::MAX);
    Bounds {
        x: area.x.saturating_add(offset(area.width - width)),
        y: area.y.saturating_add(offset(area.height - height)),
        width,
        height,
    }
}
