//! The Tauri config commands and the side effects only the Tauri client runs;
//! the values themselves live in [`tterm_core::config`].

pub(crate) mod commands;
mod save;
pub mod window_background;
pub(crate) mod window_background_commands;
