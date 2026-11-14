//! The Tauri serial commands and the protocol layers that stay beside them;
//! the serial connection itself lives in [`tterm_core::serial`].

pub(crate) mod commands;
pub(crate) mod history;
pub(crate) mod xmodem;
