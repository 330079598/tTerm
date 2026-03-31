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

pub fn build_terminal_command(
    shell_config: Option<crate::core::session::TerminalShellConfig>,
) -> Result<CommandBuilder, String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let config = shell_config.unwrap_or(crate::core::session::TerminalShellConfig {
            shell: "auto".to_string(),
            custom_path: None,
            custom_args: None,
        });

        build_windows_command(config)?
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        CommandBuilder::new(&shell)
    };

    let home = resolve_home_dir();
    cmd.env("TERM", "xterm-256color");
    cmd.env("HOME", &home);
    cmd.env("LANG", "en_US.UTF-8");
    cmd.cwd(&home);
    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn build_windows_command(
    config: crate::core::session::TerminalShellConfig,
) -> Result<CommandBuilder, String> {
    let shell = config.shell.trim().to_ascii_lowercase();

    let (program, args): (String, Vec<String>) = match shell.as_str() {
        "cmd" => ("cmd.exe".to_string(), Vec::new()),
        "powershell" => (
            "powershell.exe".to_string(),
            vec!["-NoLogo".to_string()],
        ),
        "pwsh" => ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]),
        "custom" => {
            let path = config
                .custom_path
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "Terminal shell is set to custom but no custom path was provided".to_string())?;

            let args = config
                .custom_args
                .as_ref()
                .map(|value| value.split_whitespace().map(|s| s.to_string()).collect())
                .unwrap_or_default();

            (path, args)
        }
        _ => resolve_windows_auto_shell(),
    };

    let mut cmd = CommandBuilder::new(&program);
    if !args.is_empty() {
        cmd.args(args);
    }
    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn resolve_windows_auto_shell() -> (String, Vec<String>) {
    if let Ok(comspec) = std::env::var("COMSPEC") {
        let trimmed = comspec.trim();
        if !trimmed.is_empty() {
            return (trimmed.to_string(), Vec::new());
        }
    }

    if command_exists_on_path("pwsh.exe") {
        return ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]);
    }

    if command_exists_on_path("powershell.exe") {
        return ("powershell.exe".to_string(), vec!["-NoLogo".to_string()]);
    }

    ("cmd.exe".to_string(), Vec::new())
}

#[cfg(target_os = "windows")]
fn command_exists_on_path(executable: &str) -> bool {
    let path_var = std::env::var_os("PATH");
    let Some(path_var) = path_var else {
        return false;
    };

    for dir in std::env::split_paths(&path_var) {
        if dir.join(executable).exists() {
            return true;
        }
    }

    false
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
    shell_config: Option<crate::core::session::TerminalShellConfig>,
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

    let cmd = build_terminal_command(shell_config)?;
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
