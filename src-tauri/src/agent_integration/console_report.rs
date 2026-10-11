//! tTerm run as an agent's hook on Windows (`--agent-report <agent> <state>`).
//!
//! Codex starts its hooks through `cmd /C` with a hidden console of their
//! own, so the hook cannot write the report to its output. It attaches to the
//! agent's console instead and writes it there, and ConPTY forwards it to the
//! tab like the agent's own output. The agent's console is the first one up
//! the hook's ancestors with a window: a hidden console has none, a
//! pseudoconsole has one. Without one, the agent runs its sessions in a
//! background server, and the report goes with the session's id to every
//! console running the agent, as on Unix.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, E_FAIL};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

use super::REPORT_ARG;
use crate::terminal::console_reset::{with_console, write_sequence};

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
    let session = read_session();
    report(&agent, &state, session.as_deref());
    // Like the hook on Unix, finding no console is not the hook's failure.
    Some(0)
}

fn report(agent: &str, state: &str, session: Option<&str>) {
    let processes = processes();
    let mut consoles = HashSet::new();
    let own = format!("\x1b]777;tterm-agent;{agent};{state}\x07");
    let mut seen = HashSet::new();
    let mut pid = processes.get(&std::process::id()).map(|process| process.0);
    // A parent id may have been reused; the set ends a cycle that makes.
    while let Some(current) = pid.filter(|&pid| pid != 0 && seen.insert(pid)) {
        if write_to_console(current, &own, &mut consoles) {
            return;
        }
        pid = processes.get(&current).map(|process| process.0);
    }
    let Some(session) = session else {
        return;
    };
    let addressed = format!("\x1b]777;tterm-agent;{agent};{state};{session}\x07");
    let exe = format!("{agent}.exe");
    for (pid, (_, name)) in &processes {
        if name.eq_ignore_ascii_case(&exe) {
            write_to_console(*pid, &addressed, &mut consoles);
        }
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

/// The `session_id` of the JSON the agent writes to the hook's input.
fn read_session() -> Option<String> {
    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_INPUT_BYTES)
        .read_to_end(&mut input)
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&input).ok()?;
    let session = value.get("session_id")?.as_str()?;
    let valid = !session.is_empty()
        && session.len() <= 64
        && session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    valid.then(|| session.to_string())
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
    fn finds_this_process_and_its_parent() {
        let processes = processes();
        let (parent, name) = &processes[&std::process::id()];
        assert!(processes.contains_key(parent));
        assert!(name.to_ascii_lowercase().ends_with(".exe"));
    }
}
