//! The Tauri-local parts of the terminal package: the watchers, the command
//! glue and the foreground query; the terminal itself lives in
//! [`tterm_core::terminal`].

#[cfg(unix)]
pub(crate) mod cwd_watch;
pub(crate) mod foreground;
pub(crate) mod foreground_commands;
pub(crate) mod pty_commands;
