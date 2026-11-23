//! Captures the production settings page through the headless renderer.
//!
//! This is a Debug-only, automation-feature command. It mounts the production
//! page with an in-memory configuration and no-op writers, so it never reads a
//! user config, database or credential file. It does not accept an executable.
//!
//! ```sh
//! cargo run -p tterm-gpui --example settings_capture --features tterm-gpui/automation -- \
//!   --output target/parity/settings-gpui-headless
//! ```

use std::path::PathBuf;
use std::process::exit;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut output = root.join("target/parity/settings-gpui-headless");
    let mut reference =
        root.join("docs/gpui-migration/reports/assets/settings-scrollback-flow/reference");
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                let Some(path) = arguments.next() else {
                    eprintln!("settings_capture: --output needs a path");
                    exit(2);
                };
                output = PathBuf::from(path);
            }
            "--reference" => {
                let Some(path) = arguments.next() else {
                    eprintln!("settings_capture: --reference needs a path");
                    exit(2);
                };
                reference = PathBuf::from(path);
            }
            other => {
                eprintln!("settings_capture: unknown argument {other}");
                exit(2);
            }
        }
    }
    println!(
        "settings_capture: debug-only, isolated in-memory config; output {}",
        output.display()
    );
    match tterm_gpui::terminal_demo::capture_settings_headlessly(&output, &reference) {
        Ok(()) => println!("settings_capture: wrote {}", output.display()),
        Err(error) => {
            eprintln!("settings_capture: {error}");
            exit(1);
        }
    }
}
