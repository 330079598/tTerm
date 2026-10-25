//! What each terminal tab showed, kept so that the next launch shows it
//! again above the tab's new session. Each tab has one gzip file in
//! `scrollback/` beside the database. The files stay out of the database so
//! that backups and sync never carry what a terminal printed.

use crate::core::blocking::run_blocking;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const DIR_NAME: &str = "scrollback";
const EXTENSION: &str = "gz";
/// A snapshot is a few thousand lines of text with its colors; anything this
/// large is not one, and is skipped rather than kept in memory.
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TAB_ID_LEN: usize = 128;

/// A tab's terminal content as the frontend serialized it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollbackSnapshot {
    pub tab_id: String,
    pub content: String,
}

/// A saved snapshot, with when it was written.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SavedScrollback {
    pub tab_id: String,
    pub content: String,
    /// Milliseconds since the Unix epoch.
    pub saved_at: i64,
}

fn snapshot_dir() -> Result<PathBuf, String> {
    Ok(crate::config::paths::get_config_path()?.join(DIR_NAME))
}

/// Tab ids name the files, so only the characters tTerm's own ids use are
/// accepted; anything else could reach outside the folder.
fn is_valid_tab_id(tab_id: &str) -> bool {
    !tab_id.is_empty()
        && tab_id.len() <= MAX_TAB_ID_LEN
        && tab_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn snapshot_path(dir: &Path, tab_id: &str) -> Result<PathBuf, String> {
    if !is_valid_tab_id(tab_id) {
        return Err(format!(
            "Cannot save the terminal content of tab '{tab_id}': the tab id is not valid."
        ));
    }
    Ok(dir.join(format!("{tab_id}.{EXTENSION}")))
}

/// The tab id a file in the folder belongs to, or `None` for a stray file.
fn tab_id_of(path: &Path) -> Option<&str> {
    if path.extension().and_then(|ext| ext.to_str()) != Some(EXTENSION) {
        return None;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| is_valid_tab_id(stem))
}

fn compress(content: &str) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(content.as_bytes())
        .and_then(|()| encoder.finish())
        .map_err(|error| format!("Failed to compress terminal content: {error}"))
}

fn decompress(bytes: &[u8]) -> Option<String> {
    let mut content = String::new();
    GzDecoder::new(bytes)
        .take(MAX_SNAPSHOT_BYTES)
        .read_to_string(&mut content)
        .ok()?;
    Some(content)
}

pub(crate) fn write_snapshots(dir: &Path, snapshots: &[ScrollbackSnapshot]) -> Result<(), String> {
    for snapshot in snapshots {
        let path = snapshot_path(dir, &snapshot.tab_id)?;
        crate::config::atomic::atomic_write_private(&path, compress(&snapshot.content)?)?;
    }
    Ok(())
}

fn saved_at(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// The snapshot files in `dir`, by tab id. A missing folder has none.
fn snapshot_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read saved terminal content: {error}")),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("Failed to read saved terminal content: {error}"))?
            .path();
        if let Some(tab_id) = tab_id_of(&path) {
            files.push((tab_id.to_string(), path));
        }
    }
    Ok(files)
}

/// Deletes the snapshots of tabs that are not in `keep`.
pub(crate) fn prune(dir: &Path, keep: &HashSet<String>) -> Result<(), String> {
    for (tab_id, path) in snapshot_files(dir)? {
        if !keep.contains(&tab_id) {
            remove_file(&path)?;
        }
    }
    Ok(())
}

/// Reads the snapshots of the tabs in `keep`, and deletes the others, which
/// belong to tabs closed while the last launch could not clean up after them.
pub(crate) fn read_snapshots(
    dir: &Path,
    keep: &HashSet<String>,
) -> Result<Vec<SavedScrollback>, String> {
    prune(dir, keep)?;
    let mut snapshots = Vec::new();
    for (tab_id, path) in snapshot_files(dir)? {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        // A file cut short by a crash is dropped; the tab starts empty.
        let Some(content) = decompress(&bytes) else {
            remove_file(&path)?;
            continue;
        };
        snapshots.push(SavedScrollback {
            tab_id,
            content,
            saved_at: saved_at(&path),
        });
    }
    Ok(snapshots)
}

pub(crate) fn clear(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to delete saved terminal content: {error}")),
    }
}

fn remove_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Failed to delete saved terminal content: {error}")),
    }
}

/// Deletes the snapshots of tabs that are no longer in the saved session.
pub(crate) fn prune_to_session(tabs: &serde_json::Value) -> Result<(), String> {
    let keep = tabs
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tab| tab.get("id").and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect();
    prune(&snapshot_dir()?, &keep)
}

/// Deletes every saved snapshot.
pub(crate) fn clear_all() -> Result<(), String> {
    clear(&snapshot_dir()?)
}

#[tauri::command]
pub async fn save_scrollback_snapshots(snapshots: Vec<ScrollbackSnapshot>) -> Result<(), String> {
    run_blocking(move || write_snapshots(&snapshot_dir()?, &snapshots)).await
}

/// The saved content of `tab_ids`; the snapshots of other tabs are deleted.
#[tauri::command]
pub async fn load_scrollback_snapshots(
    tab_ids: Vec<String>,
) -> Result<Vec<SavedScrollback>, String> {
    run_blocking(move || read_snapshots(&snapshot_dir()?, &tab_ids.into_iter().collect())).await
}

#[tauri::command]
pub async fn clear_scrollback_snapshots() -> Result<(), String> {
    run_blocking(clear_all).await
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("tterm-scrollback-{}", uuid::Uuid::new_v4())))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn snapshot(tab_id: &str, content: &str) -> ScrollbackSnapshot {
        ScrollbackSnapshot {
            tab_id: tab_id.to_string(),
            content: content.to_string(),
        }
    }

    fn ids(tab_ids: &[&str]) -> HashSet<String> {
        tab_ids.iter().map(|id| id.to_string()).collect()
    }

    fn read(dir: &Path, keep: &[&str]) -> Vec<(String, String)> {
        let mut snapshots: Vec<_> = read_snapshots(dir, &ids(keep))
            .unwrap()
            .into_iter()
            .map(|saved| (saved.tab_id, saved.content))
            .collect();
        snapshots.sort();
        snapshots
    }

    #[test]
    fn snapshots_round_trip_with_their_escape_sequences() {
        let dir = TempDir::new();
        let content = "\u{1b}[31mred\u{1b}[0m 中文\r\n$ ";
        write_snapshots(&dir.0, &[snapshot("tab-1", content)]).unwrap();
        let saved = read_snapshots(&dir.0, &ids(&["tab-1"])).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].content, content);
        assert!(saved[0].saved_at > 0);
    }

    #[test]
    fn a_new_snapshot_replaces_the_old_one() {
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "old")]).unwrap();
        write_snapshots(&dir.0, &[snapshot("tab-1", "new")]).unwrap();
        assert_eq!(
            read(&dir.0, &["tab-1"]),
            [("tab-1".to_string(), "new".to_string())]
        );
    }

    #[test]
    fn reading_deletes_the_snapshots_of_tabs_no_longer_open() {
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "a"), snapshot("tab-2", "b")]).unwrap();
        assert_eq!(
            read(&dir.0, &["tab-2"]),
            [("tab-2".to_string(), "b".to_string())]
        );
        assert!(!dir.0.join("tab-1.gz").exists());
    }

    #[test]
    fn pruning_keeps_open_tabs_and_ignores_stray_files() {
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "a"), snapshot("tab-2", "b")]).unwrap();
        fs::write(dir.0.join("notes.txt"), "keep me").unwrap();
        prune(&dir.0, &ids(&["tab-1"])).unwrap();
        assert!(dir.0.join("tab-1.gz").exists());
        assert!(!dir.0.join("tab-2.gz").exists());
        assert!(dir.0.join("notes.txt").exists());
    }

    #[test]
    fn a_damaged_snapshot_is_dropped() {
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "a")]).unwrap();
        fs::write(dir.0.join("tab-1.gz"), b"not gzip").unwrap();
        assert!(read(&dir.0, &["tab-1"]).is_empty());
        assert!(!dir.0.join("tab-1.gz").exists());
    }

    #[test]
    fn a_missing_folder_has_no_snapshots() {
        let dir = TempDir::new();
        assert!(read(&dir.0, &["tab-1"]).is_empty());
        prune(&dir.0, &ids(&[])).unwrap();
        clear(&dir.0).unwrap();
    }

    #[test]
    fn clearing_deletes_every_snapshot() {
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "a")]).unwrap();
        clear(&dir.0).unwrap();
        assert!(!dir.0.exists());
    }

    #[test]
    fn tab_ids_that_could_leave_the_folder_are_refused() {
        let dir = TempDir::new();
        for tab_id in ["", "../tab-1", "a/b", "a\\b", "tab.1"] {
            assert!(write_snapshots(&dir.0, &[snapshot(tab_id, "x")]).is_err());
        }
        assert!(!dir.0.exists());
    }

    #[cfg(unix)]
    #[test]
    fn snapshots_are_readable_by_their_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        write_snapshots(&dir.0, &[snapshot("tab-1", "a")]).unwrap();
        let mode = fs::metadata(dir.0.join("tab-1.gz"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
