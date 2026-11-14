//! Local and remote terminal sessions: their shells, scripts and output.

#[cfg(target_os = "windows")]
pub mod console_reset;
pub mod encoding;
pub mod io_batcher;
pub mod legacy_events;
#[cfg(unix)]
pub mod local_integration;
pub mod output;
pub mod output_tail;
pub mod pty;
pub mod reader;
#[cfg(test)]
mod reader_tests;
mod shell_args;
#[cfg(target_os = "windows")]
mod shell_integration;
pub mod shell_scripts;
pub mod ssh_query_handler;
pub mod types;
#[cfg(not(target_os = "windows"))]
mod unix_shell;
#[cfg(target_os = "windows")]
mod windows_shell;
#[cfg(target_os = "windows")]
mod wsl_distros;
