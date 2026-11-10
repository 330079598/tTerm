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

/// Shows Windows' own Open With dialog for `path`, which opens the file in
/// the program picked there. Returns `false` when the user cancelled it.
/// The dialog is modal and runs a message loop of its own on this thread.
#[cfg(windows)]
pub(super) fn open_with_system_chooser(path: &Path) -> Result<bool, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{HRESULT, PCWSTR};
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        SHOpenWithDialog, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OAIF_REGISTER_EXT, OPENASINFO,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let info = OPENASINFO {
        pcszFile: PCWSTR(wide.as_ptr()),
        pcszClass: PCWSTR::null(),
        oaifInFlags: OAIF_ALLOW_REGISTRATION | OAIF_REGISTER_EXT | OAIF_EXEC,
    };
    // SAFETY: COM is initialized for this thread for the dialog's lifetime
    // and released after; `info` points at `wide`, which outlives the call.
    let result = unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let result = SHOpenWithDialog(None, &info);
        if initialized.is_ok() {
            CoUninitialize();
        }
        result
    };
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => Ok(false),
        Err(error) => Err(format!("Failed to show Open With: {error}")),
    }
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
