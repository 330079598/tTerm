//! Soft-resets the conhost behind a local ConPTY session.
//!
//! ConPTY interprets the shell's output itself and forwards only what it
//! rendered: after `ESC ( 0` (DEC line drawing, e.g. from `cat` on a binary)
//! the frontend receives `┌─┐` instead of `lqk`, so resetting xterm.js alone
//! cannot undo it. The fix is writing DECSTR to that console's own output
//! buffer, which needs `AttachConsole`. Attaching is process-wide and exposes
//! the attached process to the console's Ctrl+C events, so tTerm runs itself
//! as a short-lived helper (`--reset-console <pid>`) to do it instead.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Console::{
    AttachConsole, FreeConsole, GetConsoleMode, SetConsoleCtrlHandler, SetConsoleMode,
    WriteConsoleW, CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
};
use windows::Win32::System::Threading::DETACHED_PROCESS;

const HELPER_ARG: &str = "--reset-console";
/// DECSTR: conhost resets its charsets, text attributes and modes, and
/// forwards the sequence so xterm.js does the same.
const SOFT_RESET: &str = "\x1b[!p";
const HELPER_TIMEOUT: Duration = Duration::from_secs(3);

/// Runs the helper when tTerm was started as one; returns its exit code.
/// Call first thing in `main`, before Tauri starts.
pub fn run_helper_if_requested() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some(HELPER_ARG) {
        return None;
    }
    let Some(pid) = args.next().and_then(|pid| pid.parse::<u32>().ok()) else {
        return Some(2);
    };
    Some(match write_soft_reset(pid) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Failed to reset console of process {pid}: {err}");
            1
        }
    })
}

/// Soft-resets the console that process `pid` (a ConPTY shell) is attached to.
pub fn reset_console(pid: u32) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| format!("Failed to locate tTerm: {err}"))?;
    let mut helper = Command::new(exe)
        .args([HELPER_ARG, &pid.to_string()])
        // No console of its own, so it can attach to the shell's.
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()
        .map_err(|err| format!("Failed to start console reset helper: {err}"))?;

    let deadline = Instant::now() + HELPER_TIMEOUT;
    loop {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("Console reset helper failed: {status}")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = helper.kill();
                return Err("Console reset helper timed out".to_string());
            }
            Err(err) => return Err(format!("Failed to wait for console reset helper: {err}")),
        }
    }
}

fn write_soft_reset(pid: u32) -> windows::core::Result<()> {
    unsafe {
        // A Ctrl+C typed into the shell while attached must not end the helper
        // midway; this process ignores it instead.
        SetConsoleCtrlHandler(None, true)?;
        let _ = FreeConsole();
        AttachConsole(pid)?;
        let result = (|| {
            let output = CreateFileW(
                w!("CONOUT$"),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )?;
            let mut mode = CONSOLE_MODE(0);
            // The mode belongs to the shell's screen buffer: enable VT only
            // for this write and put back exactly what was there.
            let written = GetConsoleMode(output, &mut mode).and_then(|()| {
                let written = SetConsoleMode(output, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING)
                    .and_then(|()| {
                        let text: Vec<u16> = SOFT_RESET.encode_utf16().collect();
                        WriteConsoleW(output, &text, None, None)
                    });
                let _ = SetConsoleMode(output, mode);
                written
            });
            let _ = CloseHandle(output);
            written
        })();
        let _ = FreeConsole();
        result
    }
}
