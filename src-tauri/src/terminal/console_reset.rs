//! Soft-resets the conhost behind a local ConPTY session.
//!
//! ConPTY interprets the shell's output itself and forwards only what it
//! rendered: after `ESC ( 0` (DEC line drawing, e.g. from `cat` on a binary)
//! the frontend receives `┌─┐` instead of `lqk`, so resetting xterm.js alone
//! cannot undo it. The fix is writing DECSTR to that console's own output
//! buffer, which needs `AttachConsole`. Attaching is process-wide and exposes
//! the attached process to the console's Ctrl+C events, so tTerm runs itself
//! as a short-lived helper (`--reset-console <pid>`) to do it instead.
//!
//! The same helper (`--flush-console-input <pid>`) also discards input the
//! console holds for its shell: conhost takes a whole paste off the PTY at
//! once and hands it to a slow shell over minutes, so cancelling a paste has
//! to empty the console's input buffer, not just tTerm's own queue.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Console::{
    AttachConsole, FlushConsoleInputBuffer, FreeConsole, GetConsoleMode, SetConsoleCtrlHandler,
    SetConsoleMode, WriteConsoleW, CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
};
use windows::Win32::System::Threading::DETACHED_PROCESS;

const RESET_ARG: &str = "--reset-console";
const FLUSH_INPUT_ARG: &str = "--flush-console-input";
/// DECSTR: conhost resets its charsets, text attributes and modes, and
/// forwards the sequence so xterm.js does the same.
const SOFT_RESET: &str = "\x1b[!p";
const HELPER_TIMEOUT: Duration = Duration::from_secs(3);

/// Runs the helper when tTerm was started as one; returns its exit code.
/// Call first thing in `main`, before Tauri starts.
pub fn run_helper_if_requested() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    let action: fn(u32) -> windows::core::Result<()> = match args.next().as_deref() {
        Some(RESET_ARG) => write_soft_reset,
        Some(FLUSH_INPUT_ARG) => flush_input,
        _ => return None,
    };
    let Some(pid) = args.next().and_then(|pid| pid.parse::<u32>().ok()) else {
        return Some(2);
    };
    Some(match action(pid) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("Failed to update console of process {pid}: {err}");
            1
        }
    })
}

/// Soft-resets the console that process `pid` (a ConPTY shell) is attached to.
pub fn reset_console(pid: u32) -> Result<(), String> {
    run_helper(RESET_ARG, pid)
}

/// Discards the input the console behind process `pid` holds but its shell
/// has not read yet.
pub fn flush_console_input(pid: u32) -> Result<(), String> {
    run_helper(FLUSH_INPUT_ARG, pid)
}

fn run_helper(action: &str, pid: u32) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| format!("Failed to locate tTerm: {err}"))?;
    let mut helper = Command::new(exe)
        .args([action, &pid.to_string()])
        // No console of its own, so it can attach to the shell's.
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()
        .map_err(|err| format!("Failed to start console helper: {err}"))?;

    let deadline = Instant::now() + HELPER_TIMEOUT;
    loop {
        match helper.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("Console helper failed: {status}")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = helper.kill();
                return Err("Console helper timed out".to_string());
            }
            Err(err) => return Err(format!("Failed to wait for console helper: {err}")),
        }
    }
}

fn write_soft_reset(pid: u32) -> windows::core::Result<()> {
    with_console(pid, w!("CONOUT$"), |output| unsafe {
        let mut mode = CONSOLE_MODE(0);
        // The mode belongs to the shell's screen buffer: enable VT only
        // for this write and put back exactly what was there.
        GetConsoleMode(output, &mut mode)?;
        let written =
            SetConsoleMode(output, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING).and_then(|()| {
                let text: Vec<u16> = SOFT_RESET.encode_utf16().collect();
                WriteConsoleW(output, &text, None, None)
            });
        let _ = SetConsoleMode(output, mode);
        written
    })
}

fn flush_input(pid: u32) -> windows::core::Result<()> {
    with_console(pid, w!("CONIN$"), |input| unsafe {
        FlushConsoleInputBuffer(input)
    })
}

/// Attaches to the console of process `pid` and runs `f` on its input or
/// output buffer, `name` being `CONIN$` or `CONOUT$`.
fn with_console(
    pid: u32,
    name: PCWSTR,
    f: impl FnOnce(HANDLE) -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    unsafe {
        // A Ctrl+C typed into the shell while attached must not end the helper
        // midway; this process ignores it instead.
        SetConsoleCtrlHandler(None, true)?;
        let _ = FreeConsole();
        AttachConsole(pid)?;
        let result = CreateFileW(
            name,
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .and_then(|handle| {
            let result = f(handle);
            let _ = CloseHandle(handle);
            result
        });
        let _ = FreeConsole();
        result
    }
}
