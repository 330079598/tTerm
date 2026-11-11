//! Native application discovery and explicit-program opening.
#[cfg(target_os = "macos")]
pub mod apps;
pub mod launch;
