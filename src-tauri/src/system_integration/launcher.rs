//! Starting tTerm from outside it: the executable that the `tterm` command and
//! the folder menu name, and the scripts that run it.

use std::path::{Path, PathBuf};

/// tTerm's executable, for a script or menu item to name. On macOS, one that
/// runs from somewhere that goes away is refused, since what names it would
/// stop working.
pub fn tterm_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("Failed to find tTerm's executable: {error}"))?;
    #[cfg(target_os = "macos")]
    if is_temporary(&exe) {
        return Err(
            "tTerm is running from a disk image or from a temporary copy macOS made of \
             it. Move tTerm to the Applications folder, open it from there and try again."
                .to_string(),
        );
    }
    Ok(exe)
}

/// Whether `exe` runs from a disk image, or from the copy at a random path
/// that Gatekeeper runs an app from when it is opened where it was downloaded
/// (App Translocation). macOS mounts both read-only.
#[cfg(target_os = "macos")]
fn is_temporary(exe: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    if exe
        .components()
        .any(|part| part.as_os_str() == "AppTranslocation")
    {
        return true;
    }
    let Ok(path) = std::ffi::CString::new(exe.as_os_str().as_bytes()) else {
        return false;
    };
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is NUL-terminated and outlives the call, and `stat` is
    // read only once the call has filled it in.
    unsafe {
        libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) == 0
            && stat.assume_init().f_flag & libc::ST_RDONLY != 0
    }
}

/// Writes `content` to `path` unless it holds that already. Returns whether
/// it wrote.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if std::fs::read_to_string(path).is_ok_and(|current| current == content) {
        return Ok(false);
    }
    std::fs::write(path, content)
        .map_err(|error| format!("Failed to write '{}': {error}", path.display()))?;
    Ok(true)
}

/// A sh script that starts `program` with the script's arguments, in the
/// directory it was run in, and returns to the prompt at once. Its output
/// goes nowhere, so the script says so itself when `program` has gone, as it
/// does when tTerm is moved or removed.
pub fn sh_launcher(program: &str) -> String {
    let program = sh_quote(program);
    format!(
        "#!/bin/sh\n\
         if [ ! -x {program} ]; then\n\
         \techo \"tTerm is no longer at \"{program}\". Open it once from its new location to fix this.\" >&2\n\
         \texit 1\n\
         fi\n\
         {program} \"$@\" </dev/null >/dev/null 2>&1 &\n"
    )
}

/// `text` as one word that sh takes literally.
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sh_launcher_quotes_the_program_for_the_shell() {
        let program = r"'/Users/o'\''neil/tTerm'";
        assert_eq!(
            sh_launcher("/Users/o'neil/tTerm"),
            format!(
                "#!/bin/sh\n\
                 if [ ! -x {program} ]; then\n\
                 \techo \"tTerm is no longer at \"{program}\". Open it once from its new location to fix this.\" >&2\n\
                 \texit 1\n\
                 fi\n\
                 {program} \"$@\" </dev/null >/dev/null 2>&1 &\n"
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_sh_launcher_reports_a_program_that_has_gone() {
        let missing = std::env::temp_dir().join(format!("tterm-gone-{}", uuid::Uuid::new_v4()));
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(sh_launcher(&missing.to_string_lossy()))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&format!("no longer at {}.", missing.display())));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_translocated_copy_is_temporary() {
        assert!(is_temporary(Path::new(
            "/private/var/folders/x/T/AppTranslocation/1234/d/tTerm.app/Contents/MacOS/tTerm"
        )));
        assert!(!is_temporary(&std::env::temp_dir()));
    }

    #[test]
    fn writes_only_what_changed() {
        let path = std::env::temp_dir().join(format!("tterm-launcher-{}", uuid::Uuid::new_v4()));
        assert_eq!(write_if_changed(&path, "a"), Ok(true));
        assert_eq!(write_if_changed(&path, "a"), Ok(false));
        assert_eq!(write_if_changed(&path, "b"), Ok(true));
        let _ = std::fs::remove_file(&path);
    }
}
