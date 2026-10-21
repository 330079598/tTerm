//! Whether a local shell on macOS or Linux is back in the foreground of its
//! terminal, which the frontend asks while a program has mouse reporting on:
//! a program killed before it could turn reporting off leaves the terminal
//! sending every mouse move to the shell's line editor as text.

use tauri::State;

use crate::core::state::{ActiveSession, PtyMap};

/// Programs that are shells, so a session leader running one again is at its
/// prompt. A login shell that `exec`ed into something else (tmux, an editor)
/// keeps the leader's pid, which is why the name is checked too.
#[cfg(unix)]
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "mksh", "yash", "tcsh", "csh", "nu", "elvish",
    "pwsh", "osh",
];

/// True when the foreground process group of the tab's local terminal is
/// its shell's own, so no program is running in it. False for SSH sessions,
/// stale sessions and on Windows, where it cannot be told.
#[tauri::command]
pub async fn pty_shell_in_foreground(
    tab_id: String,
    session_nonce: u32,
    state: State<'_, PtyMap>,
) -> Result<bool, String> {
    let map = state.read().await;
    let Some(session) = map.get(&tab_id) else {
        return Ok(false);
    };
    if session.session_nonce != session_nonce || session.pid == 0 {
        return Ok(false);
    }
    let active = session.active.lock().await;
    let Some(ActiveSession::Local(local)) = active.as_ref() else {
        return Ok(false);
    };
    Ok(shell_leads_foreground(local.master.as_ref(), session.pid))
}

#[cfg(unix)]
fn shell_leads_foreground(master: &dyn portable_pty::MasterPty, shell_pid: u32) -> bool {
    let leader = master.process_group_leader();
    leader.is_some_and(|pgid| u32::try_from(pgid).ok() == Some(shell_pid))
        && process_name(shell_pid).is_some_and(|name| is_shell(&name))
}

#[cfg(not(unix))]
fn shell_leads_foreground(_master: &dyn portable_pty::MasterPty, _shell_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn is_shell(name: &str) -> bool {
    SHELLS.contains(&name.trim_start_matches('-'))
}

#[cfg(target_os = "linux")]
fn process_name(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim_end().to_string())
}

#[cfg(target_os = "macos")]
fn process_name(pid: u32) -> Option<String> {
    let mut buffer = [0u8; 2 * libc::MAXCOMLEN + 1];
    // SAFETY: `proc_name` writes at most `buffer.len()` bytes into it.
    let written = unsafe {
        libc::proc_name(
            i32::try_from(pid).ok()?,
            buffer.as_mut_ptr().cast(),
            u32::try_from(buffer.len()).ok()?,
        )
    };
    let written = usize::try_from(written).ok().filter(|&len| len > 0)?;
    String::from_utf8(buffer.get(..written)?.to_vec()).ok()
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn process_name(_pid: u32) -> Option<String> {
    None
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    /// Polls for up to five seconds, since `spawn` can return before the
    /// child has `exec`ed, while it still carries the name of the test
    /// thread that forked it.
    #[test]
    fn names_a_running_process() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let mut name = None;
        for _ in 0..500 {
            name = process_name(child.id());
            if name.as_deref() == Some("sleep") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(name.as_deref(), Some("sleep"));
    }

    /// Polls for up to five seconds, since the shell hands the terminal to a
    /// command and takes it back on its own schedule.
    fn becomes(master: &dyn portable_pty::MasterPty, pid: u32, expected: bool) -> bool {
        (0..100).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            shell_leads_foreground(master, pid) == expected
        })
    }

    #[test]
    fn follows_a_real_shell_handing_its_terminal_to_a_command() {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};
        use std::io::{Read, Write};

        let pair = native_pty_system()
            .openpty(PtySize::default())
            .expect("open pty");
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-i");
        let mut child = pair.slave.spawn_command(cmd).expect("spawn shell");
        let pid = child.process_id().expect("shell pid");
        // The shell blocks once the PTY's buffer fills, so keep reading it.
        let mut reader = pair.master.try_clone_reader().expect("pty reader");
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while matches!(reader.read(&mut buffer), Ok(len) if len > 0) {}
        });
        let mut writer = pair.master.take_writer().expect("pty writer");
        let master = pair.master.as_ref();

        assert!(becomes(master, pid, true), "the idle shell leads");
        writer.write_all(b"sleep 30\n").unwrap();
        assert!(
            becomes(master, pid, false),
            "the command leads while it runs"
        );
        writer.write_all(b"\x03").unwrap();
        assert!(
            becomes(master, pid, true),
            "the shell leads again after Ctrl+C"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn knows_shells_by_name() {
        assert!(is_shell("zsh"));
        assert!(is_shell("-bash"));
        assert!(!is_shell("tmux"));
        assert!(!is_shell("claude"));
    }
}
