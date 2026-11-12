#[cfg(unix)]
pub(crate) mod cwd_watch;
pub(crate) mod encoding;
pub(crate) mod foreground;
pub(crate) mod foreground_commands;
pub(crate) mod io_batcher;
#[cfg(unix)]
mod local_integration;
pub(crate) mod output;
pub(crate) mod output_tail;
pub(crate) mod pty;
pub(crate) mod pty_commands;
pub(crate) mod reader;
#[cfg(test)]
mod reader_tests;
mod shell_args;
#[cfg(target_os = "windows")]
mod shell_integration;
pub(crate) mod ssh_query_handler;
pub(crate) mod types;
#[cfg(not(target_os = "windows"))]
mod unix_shell;
#[cfg(target_os = "windows")]
mod windows_shell;
#[cfg(target_os = "windows")]
mod wsl_distros;
