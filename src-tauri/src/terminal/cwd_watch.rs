//! Working directory of local shells on macOS and Linux, read straight from
//! the shell process, so no shell needs changing and every shell is covered.
//! A shell started inside the first one (a nested shell, tmux, ssh) is not
//! followed: the outer shell's directory is what gets restored.

use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CwdPayload {
    session_nonce: u32,
    cwd: String,
}

/// Emits `pty-cwd-<tab id>` whenever the shell's directory changes, until the
/// session stops or the shell exits.
pub fn spawn_cwd_watcher(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    pid: u32,
    stop_rx: watch::Receiver<bool>,
) {
    if pid == 0 {
        return;
    }
    let event_name = format!("pty-cwd-{tab_id}");
    std::thread::spawn(move || {
        let mut last: Option<String> = None;
        while !*stop_rx.borrow() {
            // Fails once the shell is gone.
            let Some(cwd) = process_cwd(pid) else {
                break;
            };
            if last.as_deref() != Some(cwd.as_str()) {
                let payload = CwdPayload {
                    session_nonce,
                    cwd: cwd.clone(),
                };
                let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
                last = Some(cwd);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    });
}

#[cfg(target_os = "linux")]
fn process_cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()?
        .into_os_string()
        .into_string()
        .ok()
}

#[cfg(target_os = "macos")]
fn process_cwd(pid: u32) -> Option<String> {
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: the buffer is a zeroed `proc_vnodepathinfo` of exactly `size` bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&mut info as *mut libc::proc_vnodepathinfo).cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    // `vip_path` is a NUL-terminated MAXPATHLEN buffer, split into rows only
    // to suit old compilers.
    let bytes: Vec<u8> = info
        .pvi_cdir
        .vip_path
        .iter()
        .flatten()
        .map(|&c| c as u8)
        .take_while(|&b| b != 0)
        .collect();
    String::from_utf8(bytes)
        .ok()
        .filter(|path| !path.is_empty())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_cwd(_pid: u32) -> Option<String> {
    None
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn reads_a_child_process_directory() {
        let dir = std::env::temp_dir().canonicalize().unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .current_dir(&dir)
            .spawn()
            .unwrap();
        let cwd = process_cwd(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(cwd.as_deref(), dir.to_str());
    }
}
