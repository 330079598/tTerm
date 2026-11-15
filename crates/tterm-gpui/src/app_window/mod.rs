//! Main-window options, persistence, appearance bootstrap and chrome.
//!
//! The prototype's [`crate::terminal_demo::window`] owns only the startup wiring and the
//! root composition. Window geometry and title-bar behavior live here so P3-07
//! can supply the title-bar slot without changing the window lifecycle.

pub mod background;
pub mod chrome;
pub mod config;
pub mod logical_bounds;
pub mod startup;
pub mod state;

/// The desktop platform this build targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    /// The platform of the running build.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}
