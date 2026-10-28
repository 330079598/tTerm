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

/// Queues the directory tTerm was started with, unless this start is a
/// restart that repeats the arguments of one that opened it already.
pub fn open_from_startup(app: &AppHandle) {
    let args: Vec<String> = std::env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    let Some(dir) = requested_dir(&args, &cwd) else {
        return;
    };
    if let Some(marker) = marker_path(app) {
        if marker.exists() {
            return;
        }
        if let Err(error) = std::fs::write(&marker, b"") {
            eprintln!("Failed to mark the start directory as opened: {error}");
        }
    }
    queue(app, dir);
}

/// Queues the directory a second launch asked for. The single-instance plugin
/// hands its command line and working directory to the running instance.
pub fn open_from_second_launch(app: &AppHandle, args: &[String], cwd: &str) {
    if let Some(dir) = requested_dir(args, Path::new(cwd)) {
        queue(app, dir);
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

/// The directory named in `args`, a command line with the program first,
/// resolved against `cwd`, where it was run. Options are skipped.
fn requested_dir(args: &[String], cwd: &Path) -> Option<LaunchDir> {
    let arg = args.iter().skip(1).find(|arg| !arg.starts_with('-'))?;
    let joined = cwd.join(arg);
    // Drops the `.` of `tterm .`, and on Windows resolves `..` as well.
    let path = std::path::absolute(&joined).unwrap_or(joined);
    Some(LaunchDir {
        exists: path.is_dir(),
        path: path.to_string_lossy().into_owned(),
    })
}

fn queue(app: &AppHandle, dir: LaunchDir) {
    app.state::<LaunchDirs>()
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(dir);
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
        assert_eq!(requested_dir(&args(&[]), &cwd), None);
        assert_eq!(requested_dir(&args(&["--flag", "-x"]), &cwd), None);
    }

    #[test]
    fn resolves_a_relative_directory_against_where_it_was_run() {
        let cwd = std::env::temp_dir();
        let expected = std::path::absolute(&cwd).unwrap();
        let dir = requested_dir(&args(&["."]), &cwd).unwrap();
        assert_eq!(Path::new(&dir.path), expected);
        assert!(dir.exists);
    }

    #[test]
    fn takes_the_first_argument_that_is_not_an_option() {
        let cwd = std::env::temp_dir();
        let target = std::path::absolute(cwd.parent().unwrap_or(&cwd)).unwrap();
        let target_arg = target.to_string_lossy().into_owned();
        let dir = requested_dir(&args(&["--flag", &target_arg, "."]), &cwd).unwrap();
        assert_eq!(Path::new(&dir.path), target);
    }

    #[test]
    fn reports_a_path_that_is_not_a_directory() {
        let cwd = std::env::temp_dir();
        let name = format!("tterm-missing-{}", uuid::Uuid::new_v4());
        let dir = requested_dir(&args(&[&name]), &cwd).unwrap();
        assert!(!dir.exists);
        assert!(dir.path.ends_with(&name));
    }
}
