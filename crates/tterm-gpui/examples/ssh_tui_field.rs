//! Opens the fixed P2-12 SSH profile with one password read from stdin.
//!
//! Debug-only and automation-feature only. This example never starts the
//! automation endpoint: it opens the normal SSH tab, injects the password in
//! memory, and hands the session to the TUI field driver. The parent must
//! create an empty output directory, seed a numbered instance, and confirm the
//! profile's host key once in a normal connection.
//!
//! ```sh
//! printf '%s\n' "$password" |
//!   TTERM_GPUI_CHECK_INSTANCE=1201 \
//!   cargo run -p tterm-gpui --example ssh_tui_field \
//!     --features tterm-gpui/automation -- --output target/field-capture
//! ```

#[cfg(all(debug_assertions, feature = "automation"))]
use std::io::{BufRead, IsTerminal, Read};
#[cfg(all(debug_assertions, feature = "automation"))]
use std::path::PathBuf;
use std::process::exit;
#[cfg(all(debug_assertions, feature = "automation"))]
use zeroize::Zeroizing;

#[cfg(all(debug_assertions, feature = "automation"))]
const USAGE: &str = "usage: ssh_tui_field --output DIR";

fn main() {
    #[cfg(not(all(debug_assertions, feature = "automation")))]
    {
        eprintln!(
            "ssh_tui_field: this example is available only in debug builds with the automation \
             feature."
        );
        exit(2);
    }

    #[cfg(all(debug_assertions, feature = "automation"))]
    {
        if let Err(error) = run() {
            eprintln!("ssh_tui_field: {error}");
            exit(1);
        }
    }
}

#[cfg(all(debug_assertions, feature = "automation"))]
fn run() -> Result<(), String> {
    let output = parse_output()?;
    let password = read_password()?;
    println!(
        "ssh_tui_field: opening the fixed SSH profile; output {}",
        output.display()
    );
    tterm_gpui::terminal_demo::run_ssh_tui_field(password, Some(output))?;
    println!("ssh_tui_field: the window closed");
    Ok(())
}

#[cfg(all(debug_assertions, feature = "automation"))]
fn parse_output() -> Result<PathBuf, String> {
    let mut arguments = std::env::args().skip(1);
    let mut output = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" => {
                let Some(path) = arguments.next() else {
                    return Err(format!("--output needs a directory; {USAGE}"));
                };
                if output.replace(PathBuf::from(path)).is_some() {
                    return Err(format!("--output was given more than once; {USAGE}"));
                }
            }
            other => return Err(format!("unknown argument '{other}'; {USAGE}")),
        }
    }
    output.ok_or_else(|| format!("--output is required; {USAGE}"))
}

/// Reads exactly one password line from a pipe, never from an echoed terminal.
#[cfg(all(debug_assertions, feature = "automation"))]
fn read_password() -> Result<Zeroizing<String>, String> {
    if std::io::stdin().is_terminal() {
        return Err(
            "Pipe the password on stdin; this command never reads an echoed terminal.".to_string(),
        );
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("Failed to read the password from stdin: {error}"))?;
    let mut trailing = Vec::new();
    std::io::stdin()
        .lock()
        .read_to_end(&mut trailing)
        .map_err(|error| format!("Failed to finish reading stdin: {error}"))?;
    if !trailing.is_empty() {
        bytes.fill(0);
        return Err("stdin must contain exactly one password line.".to_string());
    }
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    if bytes.is_empty() {
        return Err("The password from stdin is empty.".to_string());
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            let mut bytes = error.into_bytes();
            bytes.fill(0);
            return Err("The password from stdin must be UTF-8.".to_string());
        }
    };
    Ok(Zeroizing::new(text))
}
