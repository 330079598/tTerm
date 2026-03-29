use super::types::ActivePty;
use portable_pty::{CommandBuilder, PtySize};
use std::io::Read;
use std::thread;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

pub fn spawn_reader_thread(
    mut reader: Box<dyn Read + Send>,
    app: AppHandle,
    tab_id: String,
    exit_tx: mpsc::UnboundedSender<super::super::core::state::SessionExitSignal>,
) {
    thread::spawn(move || {
        let mut buf = [0u8; 65536];
        let mut pending_output = String::with_capacity(65536);

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]);
                    pending_output.push_str(data.as_ref());

                    // Flush immediately after every read so that vim redraws
                    // (e.g. gg) are visible without waiting for the next keypress.
                    // The OS PTY driver already coalesces tiny writes, so we don't
                    // need an additional batching layer here.
                    emit_pty_output(&app, &tab_id, std::mem::take(&mut pending_output));
                }
                Err(_) => break,
            }
        }

        emit_batched_output(&app, &tab_id, &mut pending_output);
        let _ = exit_tx.send(super::super::core::state::SessionExitSignal::Terminated);
    });
}

pub fn build_terminal_command() -> CommandBuilder {
    #[cfg(target_os = "windows")]
    let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());

    #[cfg(not(target_os = "windows"))]
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let home = resolve_home_dir();

    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("HOME", &home);
    cmd.env("LANG", "en_US.UTF-8");
    cmd.cwd(&home);
    cmd
}

fn resolve_home_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let userprofile = userprofile.trim();
            if !userprofile.is_empty() {
                return userprofile.to_string();
            }
        }

        if let (Ok(home_drive), Ok(home_path)) =
            (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH"))
        {
            let home_drive = home_drive.trim();
            let home_path = home_path.trim();
            if !home_drive.is_empty() && !home_path.is_empty() {
                return format!("{}{}", home_drive, home_path);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = home.trim();
            if !home.is_empty() {
                return home.to_string();
            }
        }

        return "C:\\".to_string();
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/".to_string())
    }
}

pub fn spawn_local_pty(
    rows: u16,
    cols: u16,
) -> Result<(u32, ActivePty), String> {
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let cmd = build_terminal_command();
    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn process: {}", e))?;

    let pid = child.process_id().unwrap_or(0);
    drop(pair.slave);

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;

    let active = ActivePty {
        writer,
        master: pair.master,
        child,
    };

    Ok((pid, active))
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

fn emit_batched_output(app: &AppHandle, tab_id: &str, pending_output: &mut String) {
    if pending_output.is_empty() {
        return;
    }
    emit_pty_output(app, tab_id, std::mem::take(pending_output));
}
