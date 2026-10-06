#[cfg(unix)]
mod cwd_watch;
mod encoding;
mod io_batcher;
mod output_tail;
mod pty;
#[cfg(target_os = "windows")]
mod shell_integration;
pub(crate) mod shell_scripts;
mod ssh_query_handler;
mod types;

#[cfg(unix)]
pub use cwd_watch::spawn_cwd_watcher;
pub use encoding::{OutputDecoder, TerminalEncoding};
pub use io_batcher::{OutputFlow, TerminalOutput, TerminalOutputSender};
pub use output_tail::OutputTail;
pub use pty::{
    __cmd__list_available_terminal_shells, __tauri_command_name_list_available_terminal_shells,
    list_available_terminal_shells,
};
pub use pty::{spawn_local_pty, spawn_reader_thread};
pub use ssh_query_handler::process_ssh_output_for_ui;
pub use types::ActivePty;
