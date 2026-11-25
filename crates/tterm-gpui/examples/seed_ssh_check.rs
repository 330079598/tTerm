//! Seeds the fixed P2-12 saved SSH profile into a fresh numbered debug instance.
//!
//! ```sh
//! TTERM_GPUI_CHECK_INSTANCE=1201 cargo run -p tterm-gpui --example seed_ssh_check -- HOST PORT USER
//! ```
//!
//! The example accepts only those three values. It does not accept a password,
//! key or path, open a connection, or start the client.

#[cfg(debug_assertions)]
use std::ffi::OsStr;
use std::process::exit;

#[cfg(debug_assertions)]
use tterm_gpui::check_instance::seed_ssh_check;

#[cfg(debug_assertions)]
const USAGE: &str = "usage: cargo run -p tterm-gpui --example seed_ssh_check -- HOST PORT USER";

fn main() {
    #[cfg(not(debug_assertions))]
    {
        eprintln!("seed_ssh_check: this example is available in debug builds only.");
        exit(2);
    }

    #[cfg(debug_assertions)]
    {
        run();
    }
}

#[cfg(debug_assertions)]
fn run() {
    let (host, port, username) = match parse_arguments() {
        Ok(arguments) => arguments,
        Err(error) => {
            eprintln!("seed_ssh_check: {error}");
            exit(2);
        }
    };

    match seed_ssh_check(&host, port, &username) {
        Ok(seeded) => {
            println!(
                "seed_ssh_check: wrote p2-12-remote to instance '{}' at {}",
                seeded.identifier,
                seeded.root.display()
            );
            println!(
                "seed_ssh_check: start the GPUI client with the same TTERM_GPUI_CHECK_INSTANCE, \
                 open \"Saved SSH connections\" (已保存的 SSH 连接), and choose p2-12-remote."
            );
        }
        Err(error) => {
            eprintln!("seed_ssh_check: {error}");
            exit(1);
        }
    }
}

#[cfg(debug_assertions)]
fn parse_arguments() -> Result<(String, u16, String), String> {
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let [host, port, username] = arguments.as_slice() else {
        return Err(USAGE.to_string());
    };
    let host = text(host, "HOST")?;
    let port = text(port, "PORT")?
        .parse::<u16>()
        .map_err(|_| "PORT must be an unsigned 16-bit integer.".to_string())?;
    if port == 0 {
        return Err("PORT must be between 1 and 65535.".to_string());
    }
    let username = text(username, "USER")?;
    Ok((host, port, username))
}

#[cfg(debug_assertions)]
fn text(value: &OsStr, name: &str) -> Result<String, String> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{name} must be valid Unicode."))
}
