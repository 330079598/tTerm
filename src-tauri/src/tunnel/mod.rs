//! The Tauri tunnel commands and the manager they drive; the tunnel rules
//! and transports live in [`tterm_core::tunnel`].

pub(crate) mod commands;
pub(crate) mod manager;
mod runtime;
