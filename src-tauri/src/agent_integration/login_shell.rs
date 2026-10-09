//! Variables the user exports in their shell's startup files. tTerm opened
//! from the Dock or a desktop menu does not inherit them, while the agents,
//! started from a shell, do: `CLAUDE_CONFIG_DIR` set in `~/.zshrc` moves
//! Claude Code's settings without tTerm seeing it in its own environment.

#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::sync::{mpsc, Mutex, OnceLock};
#[cfg(unix)]
use std::time::Duration;

/// Startup files may print, so the value is read between these.
#[cfg(unix)]
const MARK: &str = "__tterm_login_env__";

/// Long enough for a heavy zsh setup; a startup file waiting on something
/// must not hold the settings page.
#[cfg(unix)]
const TIMEOUT: Duration = Duration::from_secs(5);

/// `name` as an interactive login shell sets it; asked once per run.
#[cfg(unix)]
pub(super) fn variable(name: &'static str) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(value) = cache.lock().ok()?.get(name) {
        return value.clone();
    }
    let value = read_variable(name);
    cache.lock().ok()?.insert(name, value.clone());
    value
}

/// Windows programs inherit what the user sets; there are no startup files.
#[cfg(not(unix))]
pub(super) fn variable(_name: &'static str) -> Option<String> {
    None
}

#[cfg(unix)]
fn read_variable(name: &str) -> Option<String> {
    let default_shell = if cfg!(target_os = "macos") {
        "/bin/zsh"
    } else {
        "/bin/sh"
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.trim().is_empty())
        .unwrap_or_else(|| default_shell.to_string());
    // Interactive too: zsh reads ~/.zshrc and many ~/.bashrc files return
    // early unless interactive. `"$NAME"` and printf work in fish as well.
    let script = format!("printf '%s%s%s' '{MARK}' \"${name}\" '{MARK}'");
    let mut child = Command::new(shell)
        .args(["-i", "-l", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;

    // Reads until the value is complete rather than to the end: a daemon a
    // startup file starts (an ssh-agent) may hold the pipe open.
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    output.extend_from_slice(&buffer[..count]);
                    if String::from_utf8_lossy(&output).matches(MARK).count() >= 2 {
                        break;
                    }
                }
            }
        }
        let _ = sender.send(output);
    });
    let output = receiver.recv_timeout(TIMEOUT);
    let _ = child.kill();
    let _ = child.wait();
    parse(&String::from_utf8_lossy(&output.ok()?))
}

/// The text between the last two marks; none when empty.
#[cfg(unix)]
fn parse(output: &str) -> Option<String> {
    let end = output.rfind(MARK)?;
    let start = output[..end].rfind(MARK)? + MARK.len();
    Some(output[start..end].to_string()).filter(|value| !value.is_empty())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn reads_between_the_marks_past_what_startup_files_print() {
        let output = format!("Welcome!\n{MARK}/Users/me/.claude-work{MARK}");
        assert_eq!(parse(&output).as_deref(), Some("/Users/me/.claude-work"));
        assert_eq!(parse(&format!("{MARK}{MARK}")), None);
        assert_eq!(parse("no marks"), None);
    }
}
