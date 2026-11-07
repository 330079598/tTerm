//! P0-14's local PTY terminal prototype.
//!
//! With no arguments it opens a window running the platform's default shell
//! in a real PTY; `--ssh <target> [args...]` runs the system `ssh` there
//! instead. Both are prototype shortcuts: P1-12 opens local and SSH sessions
//! through `AppServices` and removes the direct portable-pty and `ssh` calls.

mod command;
mod conpty;
mod intents;
mod pipeline;
mod session;
mod window;

use command::{CommandSpec, command_from_environment};

const USAGE: &str = "usage: tterm-gpui [--ssh <target> [args...]]";

/// Opens the prototype window and runs until it closes.
pub fn run() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        None => window::run_window(command_from_environment()),
        Some("--ssh") => {
            let Some(target) = arguments.get(1).cloned() else {
                eprintln!("--ssh requires a user@host target");
                std::process::exit(2);
            };
            window::run_window(CommandSpec::Ssh {
                target,
                args: arguments.into_iter().skip(2).collect(),
            });
        }
        Some(_) => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}
