use std::path::Path;

#[cfg(windows)]
pub fn prepare_conpty() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
    use windows::core::PCWSTR;

    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("src-tauri")
        .join("resources")
        .join("conpty");
    let dll = resource_dir.join("conpty.dll");
    let open_console = resource_dir.join("OpenConsole.exe");
    if !dll.is_file() || !open_console.is_file() {
        eprintln!(
            "tterm-gpui: bundled ConPTY is missing at {}; falling back to system ConPTY; run scripts/prepare-conpty.ps1 or build the Tauri package",
            resource_dir.display()
        );
        return;
    }

    let wide: Vec<u16> = OsStr::new(&dll)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // Keep the module loaded for the process lifetime. portable-pty resolves
    // conpty.dll by module name on its first PTY creation. P1-12 replaces this
    // prototype-only setup with the core session service.
    let loaded = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) };
    if loaded.as_ref().map_or(true, |module| module.is_invalid()) {
        eprintln!(
            "tterm-gpui: failed to load bundled ConPTY at {}; falling back to system ConPTY",
            dll.display()
        );
        return;
    }
    let module_name: Vec<u16> = OsStr::new("conpty.dll")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let actual = unsafe { GetModuleHandleW(PCWSTR(module_name.as_ptr())) };
    if actual.is_ok() {
        eprintln!(
            "tterm-gpui: local terminals use bundled ConPTY ({})",
            dll.display()
        );
    } else {
        eprintln!("tterm-gpui: bundled ConPTY loaded but source could not be queried");
    }
}

#[cfg(not(windows))]
pub fn prepare_conpty() {}
