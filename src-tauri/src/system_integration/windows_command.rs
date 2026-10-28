//! The `tterm` command on Windows: launcher scripts in a directory of their
//! own, which goes on the user's PATH. The install directory itself would put
//! `uninstall` and `OpenConsole` on the PATH as well.

use std::path::Path;

use super::launcher::{sh_launcher, write_if_changed};
use super::user_path;

/// For Command Prompt and PowerShell.
const CMD_LAUNCHER: &str = "tterm.cmd";
/// For Git Bash and other MSYS shells, which run no `.cmd` file by its bare name.
const SH_LAUNCHER: &str = "tterm";

pub fn is_installed(bin: &Path) -> Result<bool, String> {
    user_path::contains(&bin.to_string_lossy())
}

/// Adds the command or removes it; returns whether it is installed now. The
/// user's PATH needs no administrator, so `_admin_prompt` goes unused.
pub fn set_installed(bin: &Path, installed: bool, _admin_prompt: &str) -> Result<bool, String> {
    if installed {
        write_launchers(bin)?;
        user_path::set(&bin.to_string_lossy(), true)?;
        return Ok(true);
    }
    user_path::set(&bin.to_string_lossy(), false)?;
    match std::fs::remove_dir_all(bin) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("Failed to delete '{}': {error}", bin.display()))
        }
        _ => Ok(false),
    }
}

/// Points the launchers at this tTerm while the command is installed. They
/// name its executable, which an install to another directory moves.
pub fn refresh(bin: &Path) -> Result<(), String> {
    if is_installed(bin)? {
        write_launchers(bin)?;
    }
    Ok(())
}

fn write_launchers(bin: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("Failed to find tTerm's executable: {error}"))?;
    std::fs::create_dir_all(bin)
        .map_err(|error| format!("Failed to create '{}': {error}", bin.display()))?;
    write_if_changed(&bin.join(CMD_LAUNCHER), &cmd_launcher(&exe))?;
    // MSYS shells take `C:/dir/tTerm.exe`.
    let msys_exe = exe.to_string_lossy().replace('\\', "/");
    write_if_changed(&bin.join(SH_LAUNCHER), &sh_launcher(&msys_exe))?;
    Ok(())
}

fn cmd_launcher(exe: &Path) -> String {
    // A batch file waits for the programs it runs, so `start` hands tTerm off
    // and the prompt comes back at once. Its first quoted argument is a title.
    let exe = exe.to_string_lossy().replace('%', "%%");
    format!("@echo off\r\nstart \"\" \"{exe}\" %*\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cmd_launcher_starts_tterm_with_the_arguments() {
        let script = cmd_launcher(Path::new(r"C:\Apps\100% tTerm\tTerm.exe"));
        assert_eq!(
            script,
            "@echo off\r\nstart \"\" \"C:\\Apps\\100%% tTerm\\tTerm.exe\" %*\r\n"
        );
    }
}
