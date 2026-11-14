//! The GPUI terminal runtime prototype.
//!
//! With no arguments it opens the configured local shell through
//! `AppServices`. `--ssh <saved-profile-id-or-name>` opens a saved connection,
//! and `--exec <program> [args...]` runs a local workload through the same
//! service path as the Tauri client.

pub mod binding;
mod command;
mod conpty;
mod intents;
mod pipeline;
mod prompt;
mod runtime;
pub mod session;
mod window;

#[cfg(all(feature = "automation", debug_assertions))]
mod automation;

use command::CommandSpec;
use runtime::GpuiRuntime;

/// Opens the prototype window and runs until it closes.
pub fn run() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let command = match CommandSpec::from_args(&arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("tterm-gpui: {error}");
            std::process::exit(2);
        }
    };
    let GpuiRuntime {
        services,
        events,
        _runtime,
    } = match runtime::start() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("tterm-gpui: {error}");
            std::process::exit(1);
        }
    };
    window::run_window(command, services, events);
    drop(_runtime);
}
