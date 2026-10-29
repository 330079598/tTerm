#[cfg(target_os = "windows")]
pub mod console_reset;
#[cfg(unix)]
pub(crate) mod cwd_watch;
pub(crate) mod encoding;
pub(crate) mod foreground;
pub(crate) mod io_batcher;
#[cfg(unix)]
mod local_integration;
pub(crate) mod output_tail;
pub(crate) mod pty;
pub(crate) mod reader;
#[cfg(target_os = "windows")]
mod shell_integration;
pub(crate) mod shell_scripts;
pub(crate) mod ssh_query_handler;
pub(crate) mod types;
#[cfg(not(target_os = "windows"))]
mod unix_shell;
