//! The GPUI terminal runtime prototype.
//!
//! With no arguments it opens the configured local shell through
//! `AppServices`. `--ssh <saved-profile-id-or-name>` opens a saved connection,
//! and `--exec <program> [args...]` runs a local workload through the same
//! service path as the Tauri client.

pub mod binding;
mod clipboard;
mod command;
mod conpty;
mod intents;
pub mod keymap;
mod pipeline;
mod prompt;
mod runtime;
pub mod session;
mod window;

#[cfg(all(feature = "automation", debug_assertions))]
mod automation;
#[cfg(all(feature = "automation", debug_assertions))]
mod field;

use command::CommandSpec;
use runtime::GpuiRuntime;

/// Writes the headless production settings capture.
///
/// This exists only in debug builds with the automation feature, and uses an
/// in-memory config with no-op writers, so it cannot touch a user profile.
#[cfg(all(debug_assertions, feature = "automation"))]
pub fn capture_settings_headlessly(
    output: &std::path::Path,
    reference: &std::path::Path,
) -> Result<(), String> {
    window::settings_page_capture::capture_headlessly(output, reference)
}

/// Writes the headless production search-bar capture.
///
/// This exists only in debug builds with the automation feature and uses a
/// synthetic in-process model, so it cannot read a user session.
#[cfg(all(debug_assertions, feature = "automation"))]
pub fn capture_search_headlessly(output: &std::path::Path) -> Result<(), String> {
    window::search::capture::capture_search_headlessly(output)
}

/// Runs the debug-only SSH field window with one stdin password.
///
/// The caller must use a numbered instance that already contains the fixed
/// check profile and its confirmed host key. The entry never starts the
/// automation endpoint, never writes the password to disk, and keeps the
/// password out of the saved tab and workspace.
#[cfg(all(debug_assertions, feature = "automation"))]
pub fn run_ssh_tui_field(
    password: zeroize::Zeroizing<String>,
    output: Option<std::path::PathBuf>,
) -> Result<(), String> {
    field::run(password, output)
}

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
