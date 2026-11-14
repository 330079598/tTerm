//! The Tauri background commands; the close, quit, exit-guard and tray
//! state they drive live in [`tterm_core::background`].

pub(crate) mod commands;
pub(crate) mod exit_guard_commands;
pub(crate) mod quit_commands;
pub(crate) mod tray;
