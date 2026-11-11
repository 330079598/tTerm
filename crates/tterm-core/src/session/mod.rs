//! Client-independent endpoints for a tab's output and lifecycle reports.

/// Synchronous output and lifecycle contracts for client adapters.
pub mod sink;

/// Test-only recording endpoint for observing session calls in order.
#[cfg(any(test, feature = "test-support"))]
pub mod recording;
