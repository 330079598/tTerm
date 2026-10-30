//! The local copies of remote files opened in programs on this computer:
//! where they live, what they are named, and how a change to one is told.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

/// Copies left behind are removed at startup once they are this old. A
/// copy is left behind when its tab closed with changes not uploaded, so it
/// stays for a while in case those changes are wanted.
const STALE_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// File types the system runs, installs or follows rather than shows when
/// they are opened with their default program. Opening a remote file like
/// that would run code from the server on this computer, so these only open
/// in a program the user picked.
const RUN_WHEN_OPENED: &[&str] = &[
    // Windows
    "exe",
    "com",
    "bat",
    "cmd",
    "ps1",
    "psm1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "msi",
    "msp",
    "scr",
    "pif",
    "lnk",
    "hta",
    "cpl",
    "reg",
    "url",
    "scf",
    "inf",
    "msc",
    "application",
    "appref-ms",
    "gadget",
    "settingcontent-ms",
    "library-ms", // macOS
    "app",
    "command",
    "tool",
    "pkg",
    "mpkg",
    "terminal",
    "workflow",
    "scpt",
    "applescript",
    "webloc",
    "inetloc",
    "fileloc", // Linux
    "desktop",
    "appimage",
    "run", // Scripts a default program may run
    "sh",
    "bash",
    "zsh",
    "csh",
    "ksh",
    "py",
    "pyw",
    "pl",
    "rb",
    "jar",
];

/// What a local file looks like from outside, compared once a second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FileStamp {
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// A local file's content, as far as telling a change goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Fingerprint {
    pub stamp: FileStamp,
    pub hash: [u8; 32],
}

/// The folder holding every local copy.
pub(super) fn edit_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map(|dir| dir.join("sftp-edit"))
        .map_err(|error| format!("Failed to find the cache folder for opened files: {error}"))
}

/// Creates the folder of one opened file, readable only by this user: the
/// copy holds the server's data.
pub(super) fn create_session_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    let dir = root.join(id);
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(&dir)
        .map_err(|error| format!("Failed to create '{}': {error}", dir.display()))?;
    Ok(dir)
}

/// Removes one opened file's folder and the copy in it.
pub(super) fn remove_session_dir(dir: &Path) {
    if let Err(error) = fs::remove_dir_all(dir) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Failed to remove '{}': {error}", dir.display());
        }
    }
}

/// Removes the copies left behind longer than [`STALE_AFTER`] ago.
pub(super) fn remove_stale(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let age = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok());
        if age.is_some_and(|age| age > STALE_AFTER) {
            remove_session_dir(&entry.path());
        }
    }
}

/// The name a remote file's copy gets here. It keeps the remote name, so
/// programs know the file by its extension; on Windows, characters and names
/// it does not allow in a file name are replaced.
pub(super) fn local_file_name(remote_name: &str, windows: bool) -> String {
    if !windows {
        return remote_name.to_string();
    }
    let replaced: String = remote_name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    // Windows drops trailing dots and spaces, which would change the name
    // and could change its extension.
    let trimmed = replaced.trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        return "file".to_string();
    }
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem[3..].chars().all(|c| c.is_ascii_digit() && c != '0'));
    if reserved {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Whether the file's default program would run it rather than show it.
pub(super) fn runs_when_opened(file_name: &str) -> bool {
    file_name.rsplit_once('.').is_some_and(|(_, extension)| {
        RUN_WHEN_OPENED
            .iter()
            .any(|risky| risky.eq_ignore_ascii_case(extension))
    })
}

/// The file's size and modification time; None when it is not there, as for
/// a moment while a program saves by replacing the file.
pub(super) fn stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    metadata.is_file().then(|| FileStamp {
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// The file's stamp and a hash of its content.
pub(super) fn fingerprint(path: &Path) -> Result<Fingerprint, String> {
    let read_error = |error| format!("Failed to read '{}': {error}", path.display());
    let file = fs::File::open(path).map_err(read_error)?;
    let metadata = file.metadata().map_err(read_error)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(read_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Fingerprint {
        stamp: FileStamp {
            size: metadata.len(),
            modified: metadata.modified().ok(),
        },
        hash: hasher.finalize().into(),
    })
}
