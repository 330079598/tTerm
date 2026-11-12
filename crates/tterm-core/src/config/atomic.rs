use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);
static ATOMIC_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Atomically write `content` to `path` via same-directory temp file + rename.
///
/// On success the target is either fully the previous contents or fully the new
/// contents — never a torn/partial JSON file from a crash mid-write.
pub fn atomic_write(path: &Path, content: impl AsRef<[u8]>) -> Result<(), String> {
    atomic_write_inner(path, content.as_ref(), false)
}

/// Atomically write an application-secret file with owner-only permissions on Unix.
pub fn atomic_write_private(path: &Path, content: impl AsRef<[u8]>) -> Result<(), String> {
    atomic_write_inner(path, content.as_ref(), true)
}

fn atomic_write_inner(path: &Path, content: &[u8], private: bool) -> Result<(), String> {
    let _lock = ATOMIC_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Atomic write lock was poisoned".to_string())?;

    if path.exists() && !is_regular_file(path)? {
        return Err(format!(
            "Refusing to replace directory '{}': expected a file",
            path.display()
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "Invalid path (missing parent directory): {}",
                path.display()
            )
        })?;

    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Invalid path (missing file name): {}", path.display()))?;

    let tmp_path = unique_temp_path(parent, file_name);

    if let Err(err) = write_temp_file(&tmp_path, content, private) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(err) = replace_file(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    Ok(())
}

fn unique_temp_path(parent: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{}.{}.{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        nanos,
        counter
    ))
}

fn is_regular_file(path: &Path) -> Result<bool, String> {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .map_err(|e| format!("Failed to inspect target '{}': {}", path.display(), e))
}

fn write_temp_file(tmp_path: &Path, content: &[u8], private: bool) -> Result<(), String> {
    let mut file = File::create(tmp_path)
        .map_err(|e| format!("Failed to create temp file '{}': {}", tmp_path.display(), e))?;
    set_temp_permissions(&file, private)?;
    file.write_all(content)
        .map_err(|e| format!("Failed to write temp file '{}': {}", tmp_path.display(), e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync temp file '{}': {}", tmp_path.display(), e))?;
    Ok(())
}

#[cfg(unix)]
fn set_temp_permissions(file: &File, private: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if private {
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to restrict temp file permissions: {}", e))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_temp_permissions(_file: &File, _private: bool) -> Result<(), String> {
    Ok(())
}

/// Replace `target` with `tmp_path` on the same filesystem. The fallback keeps
/// a backup and restores it if replacing the target fails.
fn replace_file(tmp_path: &Path, target: &Path) -> Result<(), String> {
    if !target.exists() {
        fs::rename(tmp_path, target)
            .map_err(|e| format!("Failed to move temp file to '{}': {}", target.display(), e))?;
        return sync_parent_directory(target);
    }

    // Prefer direct rename (atomic replace on Unix).
    if fs::rename(tmp_path, target).is_ok() {
        return sync_parent_directory(target);
    }

    // Windows (and rare Unix cases): move target aside, then rename temp into place.
    if !is_regular_file(target)? {
        return Err(format!(
            "Refusing to replace non-file target '{}': expected a file",
            target.display()
        ));
    }
    let backup = backup_path(target);

    fs::rename(target, &backup).map_err(|e| {
        format!(
            "Failed to move existing file aside '{}': {}",
            target.display(),
            e
        )
    })?;

    match fs::rename(tmp_path, target) {
        Ok(()) => {
            let _ = fs::remove_file(&backup);
            sync_parent_directory(target)
        }
        Err(e) => {
            let restore_error = fs::rename(&backup, target).err();
            let restore_detail = restore_error
                .map(|restore| {
                    format!(
                        "; failed to restore backup '{}': {}",
                        backup.display(),
                        restore
                    )
                })
                .unwrap_or_default();
            Err(format!(
                "Failed to replace file '{}': {}{}",
                target.display(),
                e,
                restore_detail
            ))
        }
    }
}

fn backup_path(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let unique = unique_temp_path(parent, std::ffi::OsStr::new(&name));
    unique.with_extension("bak")
}

#[cfg(unix)]
fn sync_parent_directory(target: &Path) -> Result<(), String> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| format!("Failed to sync directory '{}': {}", parent.display(), e))
}

#[cfg(not(unix))]
fn sync_parent_directory(_target: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tterm-atomic-write-{}-{}-{}",
            std::process::id(),
            nanos,
            n
        ));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn atomic_write_creates_new_file_with_full_contents() {
        let dir = test_dir();
        let path = dir.join("config.json");
        let payload = r#"{ "theme": "default", "scrollback_lines": 10000 }"#;

        atomic_write(&path, payload).expect("atomic_write should succeed");

        let read = fs::read_to_string(&path).expect("read written file");
        assert_eq!(read, payload);
        assert!(
            fs::read_dir(&dir)
                .expect("read dir")
                .filter_map(|e| e.ok())
                .all(|e| e.file_name() == *"config.json"),
            "temp files should be cleaned up"
        );

        cleanup(&dir);
    }

    #[test]
    fn atomic_write_replaces_existing_file_atomically() {
        let dir = test_dir();
        let path = dir.join("session.json");
        fs::write(&path, r#"{"tabs":[]}"#).expect("seed file");

        let next = r#"{"tabs":[{"id":"1"}],"active_tab_id":"1"}"#;
        atomic_write(&path, next).expect("replace should succeed");

        let read = fs::read_to_string(&path).expect("read replaced file");
        assert_eq!(read, next);

        cleanup(&dir);
    }

    #[test]
    fn failed_replace_keeps_existing_target_and_cleans_temp_file() {
        let dir = test_dir();
        let target = dir.join("target");
        fs::create_dir(&target).expect("create target directory");
        let marker = target.join("keep.txt");
        fs::write(&marker, "previous data").expect("seed existing target");

        assert!(atomic_write(&target, "new data").is_err());
        assert_eq!(
            fs::read_to_string(&marker).expect("read preserved target"),
            "previous data"
        );
        assert!(
            fs::read_dir(&dir)
                .expect("read dir")
                .filter_map(|entry| entry.ok())
                .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp")),
            "failed writes must not leave temp files"
        );

        cleanup(&dir);
    }

    #[test]
    fn atomic_write_creates_missing_parent_directories() {
        let dir = test_dir();
        let path = dir.join("nested").join("deep").join("profiles.json");
        let payload = "[]";

        atomic_write(&path, payload).expect("should create parents");
        assert_eq!(fs::read_to_string(&path).expect("read"), payload);

        cleanup(&dir);
    }

    #[test]
    fn atomic_write_handles_binary_and_utf8_payloads() {
        let dir = test_dir();
        let path = dir.join("vault.bin");
        let payload: Vec<u8> = (0u8..=255).collect();

        atomic_write(&path, &payload).expect("binary write");
        assert_eq!(fs::read(&path).expect("read binary"), payload);

        cleanup(&dir);
    }

    #[test]
    fn atomic_write_rejects_paths_without_file_name() {
        let err = atomic_write(Path::new("/"), b"x").expect_err("root path invalid");
        assert!(
            err.contains("Invalid path")
                || err.contains("file name")
                || err.contains("parent")
                || err.contains("directory"),
            "unexpected error: {err}"
        );
    }
}
