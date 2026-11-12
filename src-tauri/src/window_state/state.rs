//! Window geometry that outlives the window adapter.

use serde::{Deserialize, Serialize};

/// Outer position and inner size in physical pixels, the way the window
/// reports and takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Bounds {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}
