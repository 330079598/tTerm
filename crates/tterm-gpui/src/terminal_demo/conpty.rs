//! Makes the prototype use the ConPTY tTerm bundles on Windows.
//!
//! portable-pty loads `conpty.dll` by name the first time it opens a PTY:
//! beside the executable, else through the normal search, else it silently
//! uses the older system ConPTY in kernel32, which drops the color queries
//! TUIs send at startup. Loading the bundled DLL by its full path first makes
//! that lookup return it, and the DLL starts the `OpenConsole.exe` next to it.
//! An installed build leaves a complete pair beside the executable for
//! portable-pty to load; source builds preload the source tree's copy. P1-12
//! replaces this with the core session service.

/// A complete installed runtime takes precedence over the source tree.
#[cfg(any(windows, test))]
fn should_preload_source(has_dll: bool, has_console: bool) -> bool {
    !(has_dll && has_console)
}

/// Loads the bundled `conpty.dll` before the first PTY opens.
#[cfg(windows)]
pub(super) fn prepare_conpty() {
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    use windows::core::HSTRING;

    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
        && !should_preload_source(
            directory.join("conpty.dll").is_file(),
            directory.join("OpenConsole.exe").is_file(),
        )
    {
        return;
    }

    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("src-tauri")
        .join("resources")
        .join("conpty");
    let dll = directory.join("conpty.dll");
    if !dll.is_file() || !directory.join("OpenConsole.exe").is_file() {
        eprintln!(
            "tterm-gpui: the bundled ConPTY is missing at {}, so portable-pty looks for conpty.dll \
             by name and may use the system ConPTY; run scripts/prepare-conpty.ps1 or build the \
             Tauri package once",
            directory.display()
        );
        return;
    }
    // SAFETY: the argument is a NUL-terminated wide path that lives across
    // the call. The module is never freed, which keeps it loaded for the
    // process's lifetime as portable-pty expects.
    if let Err(error) = unsafe { LoadLibraryW(&HSTRING::from(dll.as_path())) } {
        eprintln!(
            "tterm-gpui: could not load the bundled ConPTY at {}: {error}",
            dll.display()
        );
    }
}

#[cfg(not(windows))]
pub(super) fn prepare_conpty() {}

/// Says once which ConPTY the PTYs got, after the first one opened.
#[cfg(windows)]
pub(super) fn log_conpty_source() {
    use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
    use windows::core::w;

    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        // SAFETY: GetModuleHandleW only looks up a module this process has
        // already loaded, and the name is a static NUL-terminated string.
        let Ok(module) = (unsafe { GetModuleHandleW(w!("conpty.dll")) }) else {
            eprintln!("tterm-gpui: local terminals use the system ConPTY");
            return;
        };
        let mut path = [0u16; 1024];
        // SAFETY: the buffer is writable for its whole length, which the
        // slice passes along, and the module handle came from this process.
        let length = unsafe { GetModuleFileNameW(Some(module), &mut path) } as usize;
        let path = String::from_utf16_lossy(&path[..length.min(path.len())]);
        eprintln!("tterm-gpui: local terminals use the ConPTY at {path}");
    });
}

#[cfg(not(windows))]
pub(super) fn log_conpty_source() {}

#[cfg(test)]
mod tests {
    use super::should_preload_source;

    #[test]
    fn only_a_complete_installed_pair_skips_source_preloading() {
        assert!(!should_preload_source(true, true));
        assert!(should_preload_source(false, true));
        assert!(should_preload_source(true, false));
        assert!(should_preload_source(false, false));
    }
}
