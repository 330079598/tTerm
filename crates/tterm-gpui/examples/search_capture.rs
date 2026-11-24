//! Captures the production terminal search bar through the headless renderer.
//!
//! Debug-only and automation-feature only. The model is synthetic, so the
//! command never opens a shell, an SSH connection or a user configuration.
//!
//! ```sh
//! cargo run -p tterm-gpui --example search_capture --features tterm-gpui/automation -- \
//!   --output "$(mktemp -d)/gpui"
//! ```

use std::path::PathBuf;
use std::process::exit;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut output = root.join("target/parity/search-gpui-headless");
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                let Some(path) = arguments.next() else {
                    eprintln!("search_capture: --output needs a path");
                    exit(2);
                };
                output = PathBuf::from(path);
            }
            other => {
                eprintln!("search_capture: unknown argument {other}");
                exit(2);
            }
        }
    }
    println!(
        "search_capture: debug-only, synthetic model; output {}",
        output.display()
    );
    match tterm_gpui::terminal_demo::capture_search_headlessly(&output) {
        Ok(()) => println!("search_capture: wrote {}", output.display()),
        Err(error) => {
            eprintln!("search_capture: {error}");
            exit(1);
        }
    }
}
