//! Opens a local terminal in the directory tTerm is started with, the way
//! `tterm .` in a shell or a file manager's "Open in tTerm" asks for one.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Tells the frontend that [`take_launch_dirs`] has directories for it.
const LAUNCH_DIRS_EVENT: &str = "launch-dirs";

/// Kept while an instance started with a directory runs. A restart, after an
/// update or from the app, starts tTerm with the arguments it was started
/// with; finding this file tells it that the directory was opened already.
const STARTED_WITH_DIR_MARKER: &str = "started-with-dir";

/// A directory tTerm was asked to open a terminal in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchDir {
    path: String,
    /// Whether the path is a directory a shell can start in.
    exists: bool,
}

/// The directories waiting for the frontend to open them.
#[derive(Default)]
pub struct LaunchDirs(Mutex<Vec<LaunchDir>>);

/// Takes the directories waiting to be opened. The frontend calls it once it
/// has restored the session and again on every [`LAUNCH_DIRS_EVENT`].
#[tauri::command]
pub fn take_launch_dirs(state: tauri::State<'_, LaunchDirs>) -> Vec<LaunchDir> {
    std::mem::take(&mut *state.0.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Queues the directories tTerm was started with, unless this start is a
/// restart that repeats the arguments of one that opened them already.
pub fn open_from_startup(app: &AppHandle) {
    let args: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let dirs = requested_dirs(&args, &cwd);
    if dirs.is_empty() {
        return;
    }
    if let Some(marker) = marker_path(app) {
        if marker.exists() {
            return;
        }
        if let Err(error) = std::fs::write(&marker, b"") {
            eprintln!("Failed to mark the start directory as opened: {error}");
        }
    }
    queue(app, dirs);
}

/// Queues the directories a second launch asked for. The single-instance
/// plugin hands its command line and working directory to the running instance.
pub fn open_from_second_launch(app: &AppHandle, args: &[String], cwd: &str) {
    let dirs = requested_dirs(args, Path::new(cwd));
    if !dirs.is_empty() {
        queue(app, dirs);
    }
}

/// Lets the next start open its directory again. Called when tTerm quits; a
/// restart skips it, and a crash leaves the marker for one start to skip.
pub fn forget_startup_dir(app: &AppHandle) {
    let Some(marker) = marker_path(app) else {
        return;
    };
    if let Err(error) = std::fs::remove_file(&marker) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Failed to remove '{}': {error}", marker.display());
        }
    }
}

/// The directories named in `args`, a command line with the program first,
/// resolved against `cwd`, where it was run. Options are skipped, but after
/// `--` every argument is a directory, as in `tterm -- -project`; a file
/// manager passes every folder chosen in it.
fn requested_dirs(args: &[String], cwd: &Path) -> Vec<LaunchDir> {
    let mut options_ended = false;
    args.iter()
        .skip(1)
        .filter(|arg| {
            if options_ended {
                return true;
            }
            if *arg == "--" {
                options_ended = true;
                return false;
            }
            !arg.starts_with('-')
        })
        .map(|arg| {
            let joined = cwd.join(unquoted(arg));
            // Drops the `.` of `tterm .`, and on Windows resolves `..` as well.
            let path = std::path::absolute(&joined).unwrap_or(joined);
            LaunchDir {
                exists: path.is_dir(),
                path: path.to_string_lossy().into_owned(),
            }
        })
        .collect()
}

/// Undoes a quirk of Windows command lines: in `"D:\"`, the backslash escapes
/// the closing quote, which arrives as `D:"`. No Windows path holds a quote.
#[cfg(target_os = "windows")]
fn unquoted(arg: &str) -> String {
    match arg.strip_suffix('"') {
        Some(path) => format!("{path}\\"),
        None => arg.to_string(),
    }
}

#[cfg(not(target_os = "windows"))]
fn unquoted(arg: &str) -> String {
    arg.to_string()
}

fn queue(app: &AppHandle, dirs: Vec<LaunchDir>) {
    app.state::<LaunchDirs>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(dirs);
    // Before the frontend listens, the directories wait for it to take them.
    if let Err(error) = app.emit(LAUNCH_DIRS_EVENT, ()) {
        eprintln!("Failed to announce a directory to open: {error}");
    }
}

fn marker_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(STARTED_WITH_DIR_MARKER))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        std::iter::once("tTerm")
            .chain(list.iter().copied())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn opens_nothing_without_a_directory() {
        let cwd = std::env::temp_dir();
        assert_eq!(requested_dirs(&args(&[]), &cwd), []);
        assert_eq!(requested_dirs(&args(&["--flag", "-x"]), &cwd), []);
    }

    #[test]
    fn resolves_a_relative_directory_against_where_it_was_run() {
        let cwd = std::env::temp_dir();
        let expected = std::path::absolute(&cwd).unwrap();
        let dirs = requested_dirs(&args(&["."]), &cwd);
        assert_eq!(dirs.len(), 1);
        assert_eq!(Path::new(&dirs[0].path), expected);
        assert!(dirs[0].exists);
    }

    #[test]
    fn takes_every_argument_that_is_not_an_option() {
        let cwd = std::env::temp_dir();
        let parent = std::path::absolute(cwd.parent().unwrap_or(&cwd)).unwrap();
        let parent_arg = parent.to_string_lossy().into_owned();
        let dirs = requested_dirs(&args(&["--flag", &parent_arg, "."]), &cwd);
        let paths: Vec<_> = dirs.iter().map(|dir| Path::new(&dir.path)).collect();
        assert_eq!(
            paths,
            [
                parent.as_path(),
                std::path::absolute(&cwd).unwrap().as_path()
            ]
        );
    }

    #[test]
    fn takes_every_argument_after_a_double_dash() {
        let cwd = std::env::temp_dir();
        let dirs = requested_dirs(&args(&["-x", "--", "-project", "--"]), &cwd);
        let paths: Vec<_> = dirs.iter().map(|dir| PathBuf::from(&dir.path)).collect();
        let base = std::path::absolute(&cwd).unwrap();
        assert_eq!(paths, [base.join("-project"), base.join("--")]);
    }

    #[test]
    fn reports_a_path_that_is_not_a_directory() {
        let cwd = std::env::temp_dir();
        let name = format!("tterm-missing-{}", uuid::Uuid::new_v4());
        let dirs = requested_dirs(&args(&[&name]), &cwd);
        assert!(!dirs[0].exists);
        assert!(dirs[0].path.ends_with(&name));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reads_a_drive_whose_closing_quote_was_escaped() {
        let cwd = std::env::temp_dir();
        let drive = cwd.to_string_lossy()[..2].to_string();
        let dirs = requested_dirs(&args(&[&format!("{drive}\"")]), &cwd);
        assert_eq!(dirs[0].path, format!("{drive}\\"));
        assert!(dirs[0].exists);
    }
}
