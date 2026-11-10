//! Client-independent endpoints for a tab's output and lifecycle reports.

pub mod sink;

#[cfg(any(test, feature = "test-support"))]
pub mod recording;
