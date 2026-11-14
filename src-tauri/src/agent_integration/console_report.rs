//! tTerm run as an agent's hook on Windows (`--agent-report <agent> <state>`).
//!
//! Codex starts its hooks through `cmd /C` with a hidden console of their
//! own, so the hook cannot write the report to its output. It attaches to the
//! agent's console instead and writes it there, and ConPTY forwards it to the
//! tab like the agent's own output. The agent's console is the first one up
//! the hook's ancestors with a window: a hidden console has none, a
//! pseudoconsole has one, up to the nearest process of the agent. Without
//! one, the agent runs its sessions in a background server, and the report
//! goes with the session's id to every console running the agent, as on Unix.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use windows::Win32::Foundation::{CloseHandle, E_FAIL};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::core::w;

use crate::agent_integration::REPORT_ARG;
use tterm_core::terminal::console_reset::{with_console, write_sequence};

/// The most of the hook's input read for the session's id.
const MAX_INPUT_BYTES: u64 = 1 << 20;

/// Reports for the agent when tTerm was started as its hook; returns the exit
/// code. Call first thing in `main`, before Tauri starts.
pub fn run_helper_if_requested() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(REPORT_ARG) {
        return None;
    }
    let (Some(agent), Some(state)) = (args.next(), args.next()) else {
        return Some(2);
    };
    if !is_name(&agent, 32) || !is_name(&state, 16) {
        return Some(2);
    }
    // Read even when the id goes unused: the agent writes it to the hook.
    let metadata = read_metadata();
    report(
        &agent,
        &state,
        metadata.session.as_deref(),
        metadata.cwd.as_deref(),
    );
    // Like the hook on Unix, finding no console is not the hook's failure.
    Some(0)
}

fn report(agent: &str, state: &str, session: Option<&str>, cwd: Option<&str>) {
    let processes = processes();
    let mut consoles = HashSet::new();
    let own = report_sequence(agent, state, session, cwd, false);
    let exe = format!("{agent}.exe");
    let mut seen = HashSet::new();
    let mut pid = processes.get(&std::process::id()).map(|process| process.0);
    // A parent id may have been reused; the set ends a cycle that makes.
    while let Some(current) = pid.filter(|&pid| pid != 0 && seen.insert(pid)) {
        if write_to_console(current, &own, &mut consoles) {
            return;
        }
        let Some((parent, name)) = processes.get(&current) else {
            break;
        };
        // The nearest agent runs the hook. Without a console it is the
        // background server, whose parent may be the agent that started it.
        if name.eq_ignore_ascii_case(&exe) {
            break;
        }
        pid = Some(*parent);
    }
    let Some(session) = session else {
        return;
    };
    let addressed = report_sequence(agent, state, Some(session), cwd, true);
    for (pid, (_, name)) in &processes {
        if name.eq_ignore_ascii_case(&exe) {
            write_to_console(*pid, &addressed, &mut consoles);
        }
    }
}

/// Scope distinguishes a report from this console from the shared server's fanout.
fn report_sequence(
    agent: &str,
    state: &str,
    session: Option<&str>,
    cwd: Option<&str>,
    broadcast: bool,
) -> String {
    let scope = if broadcast { "broadcast" } else { "direct" };
    let cwd = cwd
        .map(|cwd| {
            percent_encoding::utf8_percent_encode(cwd, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
        })
        .unwrap_or_default();
    match session {
        Some(session) => {
            format!("\x1b]777;tterm-agent;{agent};{state};{session};{scope};{cwd}\x07")
        }
        None => format!("\x1b]777;tterm-agent;{agent};{state}\x07"),
    }
}

/// Writes `sequence` to the console process `pid` runs in if that console
/// has a window, once per console. Returns whether it has one.
fn write_to_console(pid: u32, sequence: &str, consoles: &mut HashSet<usize>) -> bool {
    let mut found = false;
    let _ = with_console(pid, w!("CONOUT$"), |output| {
        let window = unsafe { GetConsoleWindow() };
        if window.is_invalid() {
            return Err(E_FAIL.into());
        }
        found = true;
        if !consoles.insert(window.0 as usize) {
            return Ok(());
        }
        write_sequence(output, sequence)
    });
    found
}

/// Every process's parent and executable name, by id.
fn processes() -> HashMap<u32, (u32, String)> {
    let mut processes = HashMap::new();
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return processes;
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut next = unsafe { Process32FirstW(snapshot, &mut entry) };
    while next.is_ok() {
        let length = entry
            .szExeFile
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        processes.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
        next = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    let _ = unsafe { CloseHandle(snapshot) };
    processes
}

#[derive(Default)]
struct HookMetadata {
    session: Option<String>,
    cwd: Option<String>,
}

fn read_metadata() -> HookMetadata {
    let mut input = Vec::new();
    if std::io::stdin()
        .take(MAX_INPUT_BYTES)
        .read_to_end(&mut input)
        .is_err()
    {
        return HookMetadata::default();
    }
    parse_metadata(&input)
}

/// Never interpolate raw JSON into an OSC; validate then encode its directory.
fn parse_metadata(input: &[u8]) -> HookMetadata {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(input) else {
        return HookMetadata::default();
    };
    let session = value
        .get("session_id")
        .and_then(|value| value.as_str())
        .filter(|session| {
            !session.is_empty()
                && session.len() <= 64
                && session
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
        .map(str::to_string);
    let cwd = value
        .get("cwd")
        .and_then(|value| value.as_str())
        .filter(|cwd| !cwd.is_empty() && cwd.len() <= 16384 && !cwd.chars().any(char::is_control))
        .map(str::to_string);
    HookMetadata { session, cwd }
}

/// What may go into the report: the frontend reads only these.
fn is_name(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_names_the_frontend_reads() {
        assert!(is_name("codex", 32));
        assert!(is_name("waiting", 16));
        assert!(!is_name("", 16));
        assert!(!is_name("Codex", 32));
        assert!(!is_name("done\x07\x1b]0;x", 16));
        assert!(!is_name("processing-and-more", 16));
    }

    #[test]
    fn both_agents_report_their_session_to_their_own_console() {
        for agent in ["claude", "codex"] {
            assert_eq!(
                report_sequence(agent, "processing", Some("abc-123"), None, false),
                format!("\x1b]777;tterm-agent;{agent};processing;abc-123;direct;\x07")
            );
        }
        assert_eq!(
            report_sequence("codex", "done", Some("abc-123"), None, true),
            "\x1b]777;tterm-agent;codex;done;abc-123;broadcast;\x07"
        );
        assert_eq!(
            report_sequence("claude", "done", None, None, false),
            "\x1b]777;tterm-agent;claude;done\x07"
        );
    }

    #[test]
    fn hook_json_cwd_is_validated_and_encoded_with_session() {
        let metadata = parse_metadata(br#"{"session_id":"abc-123","cwd":"C:\\work dir;name"}"#);
        assert_eq!(metadata.session.as_deref(), Some("abc-123"));
        assert_eq!(
            report_sequence(
                "claude",
                "done",
                metadata.session.as_deref(),
                metadata.cwd.as_deref(),
                false
            ),
            "\x1b]777;tterm-agent;claude;done;abc-123;direct;C%3A%5Cwork%20dir%3Bname\x07"
        );
        for cwd in [
            "/bad\nline",
            "/bad\rline",
            "/bad\tline",
            "/bad\0line",
            "/bad\u{1b}line",
            "/bad\u{85}line",
        ] {
            let input = serde_json::json!({"session_id":"abc-123", "cwd":cwd});
            assert!(parse_metadata(input.to_string().as_bytes()).cwd.is_none());
        }
        assert!(
            parse_metadata(br#"{"session_id":"abc; rm -rf ~"}"#)
                .session
                .is_none()
        );
        assert!(parse_metadata(b"{").session.is_none());
    }

    #[test]
    fn finds_this_process_and_its_parent() {
        let processes = processes();
        let (parent, name) = &processes[&std::process::id()];
        assert!(processes.contains_key(parent));
        assert!(name.to_ascii_lowercase().ends_with(".exe"));
    }
}
