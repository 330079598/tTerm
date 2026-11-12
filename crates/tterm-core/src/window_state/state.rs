//! Window geometry that outlives the window adapter.

use serde::{Deserialize, Serialize};

/// Outer position and inner size in physical pixels, the way the window
/// reports and takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
