//! Local and remote terminal sessions: their shells, scripts and output.

#[cfg(target_os = "windows")]
pub mod console_reset;
pub mod io_batcher;
pub mod legacy_events;
pub mod shell_scripts;
