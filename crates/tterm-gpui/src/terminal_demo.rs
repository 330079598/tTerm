//! P0-14's local PTY terminal prototype.
//!
//! With no arguments it opens a window running the platform's default shell
//! in a real PTY. `--ssh <target> [args...]` runs the system `ssh` there
//! instead, and `--exec <program> [args...]` any program, which is how the
//! P0 cards run fixed workloads in it. These are prototype shortcuts: P1-12
//! opens local and SSH sessions through `AppServices` and removes the direct
//! portable-pty and `ssh` calls.

mod command;
mod conpty;
mod intents;
mod pipeline;
mod session;
mod window;

#[cfg(test)]
mod tests;

use command::CommandSpec;

/// Opens the prototype window and runs until it closes.
pub fn run() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match CommandSpec::from_args(&arguments) {
        Ok(command) => window::run_window(command),
        Err(error) => {
            eprintln!("tterm-gpui: {error}");
            std::process::exit(2);
        }
    }
}
