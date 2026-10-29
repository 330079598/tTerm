//! The `tterm` command on macOS: a launcher script in tTerm's data directory,
//! linked from /usr/local/bin, which shells have on their PATH. Changing that
//! directory may take an administrator's password, which macOS asks for.

use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use super::launcher::{sh_launcher, sh_quote, write_if_changed};

const LINK: &str = "/usr/local/bin/tterm";
const LAUNCHER: &str = "tterm";

pub fn is_installed(bin: &Path) -> Result<bool, String> {
    Ok(std::fs::read_link(LINK).is_ok_and(|target| target == bin.join(LAUNCHER)))
}

/// Adds the command or removes it; returns whether it is installed now. That
/// stays as it was when the user cancels the password prompt, which shows
/// `admin_prompt`.
pub fn set_installed(bin: &Path, installed: bool, admin_prompt: &str) -> Result<bool, String> {
    let was_installed = is_installed(bin)?;
    if installed {
        write_launcher(bin)?;
        return if was_installed {
            Ok(true)
        } else {
            link(&bin.join(LAUNCHER), admin_prompt)
        };
    }
    // Another program's `tterm` is not tTerm's to remove.
    if was_installed && !unlink(admin_prompt)? {
        return Ok(true);
    }
    match std::fs::remove_dir_all(bin) {
        Err(error) if error.kind() != ErrorKind::NotFound => {
            Err(format!("Failed to delete '{}': {error}", bin.display()))
        }
        _ => Ok(false),
    }
}

/// Points the launcher at this tTerm while the command is installed. It names
/// tTerm's executable, which moving the app moves.
pub fn refresh(bin: &Path) -> Result<(), String> {
    if is_installed(bin)? {
        write_launcher(bin)?;
    }
    Ok(())
}

fn write_launcher(bin: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("Failed to find tTerm's executable: {error}"))?;
    std::fs::create_dir_all(bin)
        .map_err(|error| format!("Failed to create '{}': {error}", bin.display()))?;
    let launcher = bin.join(LAUNCHER);
    write_if_changed(&launcher, &sh_launcher(&exe.to_string_lossy()))?;
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("Failed to make '{}' runnable: {error}", launcher.display()))
}

/// Links [`LINK`] to `launcher`; returns false when the user cancels.
fn link(launcher: &Path, admin_prompt: &str) -> Result<bool, String> {
    if let Some(error) = conflict(Path::new(LINK)) {
        return Err(error);
    }
    let direct = match std::fs::remove_file(LINK) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
        _ => std::os::unix::fs::symlink(launcher, LINK),
    };
    match direct {
        Ok(()) => Ok(true),
        // The directory is root's, or missing on a Mac without Homebrew.
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::PermissionDenied | ErrorKind::NotFound
            ) =>
        {
            let target = sh_quote(&launcher.to_string_lossy());
            let script = format!("mkdir -p /usr/local/bin && ln -sfn {target} {LINK}");
            run_as_admin(&script, admin_prompt)
        }
        Err(error) => Err(format!("Failed to create {LINK}: {error}")),
    }
}

/// Why `link`, which is not tTerm's own link to this user's launcher, may not
/// be replaced, if it may not. Only a link to nothing may: anything else is
/// another program's, another user's or another copy of tTerm's, such as a
/// development build. A link this user cannot follow is another user's.
fn conflict(link: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(link).ok()?;
    if !meta.file_type().is_symlink() {
        return Some(format!(
            "{} already exists and does not belong to tTerm. Remove it to add the command.",
            link.display()
        ));
    }
    if std::fs::metadata(link).is_err_and(|error| error.kind() == ErrorKind::NotFound) {
        return None;
    }
    let target = std::fs::read_link(link).map_or_else(
        |_| "somewhere else".to_string(),
        |target| format!("'{}'", target.display()),
    );
    Some(format!(
        "{} already links to {target}. Remove it to add the command.",
        link.display()
    ))
}

/// Removes [`LINK`]; returns false when the user cancels.
fn unlink(admin_prompt: &str) -> Result<bool, String> {
    match std::fs::remove_file(LINK) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            run_as_admin(&format!("rm -f {LINK}"), admin_prompt)
        }
        Err(error) => Err(format!("Failed to remove {LINK}: {error}")),
    }
}

/// Runs `script` with sh as root once macOS has the password of an
/// administrator, asked for with `prompt`; returns false when the user
/// cancels.
fn run_as_admin(script: &str, prompt: &str) -> Result<bool, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "do shell script {} with prompt {} with administrator privileges",
            applescript_string(script),
            applescript_string(prompt)
        ))
        .output()
        .map_err(|error| format!("Failed to ask for an administrator's password: {error}"))?;
    if output.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    // AppleScript's "User canceled." error.
    if stderr.contains("(-128)") {
        return Ok(false);
    }
    Err(format!("Failed to change {LINK}: {}", stderr.trim()))
}

/// `text` as an AppleScript string literal.
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', r"\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_link_to_nothing_may_be_replaced() {
        let dir = std::env::temp_dir().join(format!("tterm-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join("tterm");
        assert_eq!(conflict(&link), None);

        std::os::unix::fs::symlink(dir.join("gone"), &link).unwrap();
        assert_eq!(conflict(&link), None);

        std::fs::write(dir.join("other"), "").unwrap();
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(dir.join("other"), &link).unwrap();
        assert!(conflict(&link).is_some_and(|error| error.contains("already links to")));

        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, "").unwrap();
        assert!(conflict(&link).is_some_and(|error| error.contains("does not belong to tTerm")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applescript_strings_escape_quotes_and_backslashes() {
        assert_eq!(applescript_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
