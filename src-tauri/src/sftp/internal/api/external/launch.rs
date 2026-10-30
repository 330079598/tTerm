//! Opening a local copy in its default program or in one the user picked.

use std::path::Path;
use std::process::{Command, Stdio};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub(super) fn open_default(app: &AppHandle, path: &Path) -> Result<(), String> {
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| format!("Failed to open '{}': {error}", path.display()))
}

/// Opens `path` in `program`: an application name or bundle on macOS, an
/// executable anywhere. The opener plugin is not used for this because on
/// Windows it passes the path to the program unquoted, so a path with a
/// space in it reaches the program as several arguments.
pub(super) fn open_with(program: &str, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if !Path::new(program).is_file() {
        return open_macos_application(program, path);
    }
    spawn_detached(program, path)
}

/// `open -a`, which finds an application by name or bundle path and fails
/// at once when there is none.
#[cfg(target_os = "macos")]
fn open_macos_application(application: &str, path: &Path) -> Result<(), String> {
    let output = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(application)
        .arg("--")
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Failed to run open: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if message.is_empty() {
        format!("Failed to open the file in '{application}'")
    } else {
        message
    })
}

fn spawn_detached(program: &str, path: &Path) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // A program started through a `.cmd` shim, such as VS Code's `code`,
    // would otherwise flash a console window.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("The program '{program}' was not found. Check the external editor in Settings.")
        } else {
            format!("Failed to start '{program}': {error}")
        }
    })?;
    // Reap the child when it exits, so it does not linger as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
