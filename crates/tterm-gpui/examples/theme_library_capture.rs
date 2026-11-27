//! Captures the production theme-library gallery headlessly.
//!
//! ```sh
//! cargo run -p tterm-gpui --example theme_library_capture \
//!   --features tterm-gpui/automation -- \
//!   --output target/parity/p4-14-live-theme-library/gallery
//! ```
//!
//! The capture uses in-memory defaults and no-op writers. It never reads a
//! user config, database, credential or desktop appearance.

use std::path::PathBuf;

fn main() {
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => output = arguments.next().map(PathBuf::from),
            other => {
                eprintln!("theme_library_capture: unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(output) = output else {
        eprintln!("theme_library_capture: pass --output DIR");
        std::process::exit(2);
    };
    println!(
        "theme_library_capture: isolated in-memory config; output {}",
        output.display()
    );
    match tterm_gpui::terminal_demo::capture_theme_library_headlessly(&output) {
        Ok(()) => println!("theme_library_capture: wrote {}", output.display()),
        Err(error) => {
            eprintln!("theme_library_capture: {error}");
            std::process::exit(1);
        }
    }
}
