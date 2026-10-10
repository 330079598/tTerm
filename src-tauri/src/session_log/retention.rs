//! Automatic cleanup of old terminal logs. Only files tTerm wrote are
//! touched: the log directory can be any folder the user picked, so a file
//! is removed only when its name looks like a terminal log and its first line
//! is a tTerm log header.

use flate2::read::GzDecoder;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const LOG_SUFFIXES: [&str; 4] = [".log", ".tlog", ".log.gz", ".tlog.gz"];
const PLAIN_HEADER: &str = "# tTerm session log";
const MAX_HEADER_BYTES: u64 = 64 * 1024;
/// Files changed this recently are left alone: they may still be closing or
/// compressing.
const RECENT: Duration = Duration::from_secs(5 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Retention {
    /// Remove logs last written more than this many days ago; 0 keeps them.
    pub max_age_days: u32,
    /// Remove the oldest logs while all of them take more than this; 0 is no
    /// limit.
    pub max_total_bytes: u64,
}

impl Retention {
    pub(super) fn is_off(&self) -> bool {
        self.max_age_days == 0 && self.max_total_bytes == 0
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Cleanup {
    pub removed: usize,
    pub freed_bytes: u64,
}

struct LogFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Removes logs in `directory` that `retention` no longer keeps. Files in
/// `in_use` (open or compressing) count toward the total but stay.
pub(super) fn clean(
    directory: &Path,
    retention: Retention,
    in_use: &HashSet<PathBuf>,
    now: SystemTime,
) -> Result<Cleanup, String> {
    let mut report = Cleanup::default();
    if retention.is_off() {
        return Ok(report);
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(error) => {
            return Err(format!(
                "Failed to read terminal log directory '{}': {error}",
                directory.display()
            ))
        }
    };

    let mut total: u64 = 0;
    let mut removable = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() || !has_log_suffix(&path) {
            continue;
        }
        let modified = metadata.modified().unwrap_or(now);
        let held =
            in_use.contains(&path) || now.duration_since(modified).unwrap_or_default() < RECENT;
        if !held && !is_tterm_log(&path) {
            continue;
        }
        total += metadata.len();
        if !held {
            removable.push(LogFile {
                path,
                size: metadata.len(),
                modified,
            });
        }
    }
    removable.sort_by_key(|file| file.modified);

    let max_age = (retention.max_age_days > 0).then(|| DAY * retention.max_age_days);
    for file in removable {
        let expired = max_age
            .is_some_and(|max_age| now.duration_since(file.modified).unwrap_or_default() > max_age);
        let over_limit = retention.max_total_bytes > 0 && total > retention.max_total_bytes;
        if !expired && !over_limit {
            // Sorted oldest first: nothing newer is expired either, and the
            // total is already within the limit.
            break;
        }
        match fs::remove_file(&file.path) {
            Ok(()) => {
                total = total.saturating_sub(file.size);
                report.removed += 1;
                report.freed_bytes += file.size;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to remove old terminal log '{}': {error}",
                    file.path.display()
                ))
            }
        }
    }
    Ok(report)
}

fn has_log_suffix(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| LOG_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)))
}

/// Whether the file starts with the header every tTerm log (and each of its
/// parts) begins with.
fn is_tterm_log(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let compressed = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".gz"));
    let reader: Box<dyn Read> = if compressed {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut first_line = String::new();
    if BufReader::new(reader.take(MAX_HEADER_BYTES))
        .read_line(&mut first_line)
        .is_err()
    {
        return false;
    }
    if first_line.starts_with(PLAIN_HEADER) {
        return true;
    }
    serde_json::from_str::<serde_json::Value>(first_line.trim_end()).is_ok_and(|header| {
        header["type"] == "header" && header["metadata"]["sessionNonce"].is_number()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("tterm-log-retention-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self, name: &str, content: &[u8], age: Duration) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, content).unwrap();
            set_age(&path, age);
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn set_age(path: &Path, age: Duration) {
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    const PLAIN: &[u8] = b"# tTerm session log\n# type=ssh\nhello\n";
    const RAW: &[u8] =
        b"{\"metadata\":{\"sessionNonce\":1,\"tabId\":\"t\"},\"type\":\"header\",\"version\":1}\n";

    fn days(count: u64) -> Duration {
        DAY * count as u32
    }

    #[test]
    fn removes_expired_tterm_logs_only() {
        let dir = TempDir::new();
        let old_plain = dir.file("a.log", PLAIN, days(10));
        let old_raw = dir.file("a.tlog", RAW, days(10));
        let fresh = dir.file("b.log", PLAIN, days(1));
        let foreign = dir.file("system.log", b"kernel: boot\n", days(30));
        let other = dir.file("notes.txt", b"# tTerm session log\n", days(30));

        let report = clean(
            &dir.0,
            Retention {
                max_age_days: 7,
                max_total_bytes: 0,
            },
            &HashSet::new(),
            SystemTime::now(),
        )
        .unwrap();

        assert_eq!(report.removed, 2);
        assert!(!old_plain.exists() && !old_raw.exists());
        assert!(fresh.exists() && foreign.exists() && other.exists());
    }

    #[test]
    fn recognizes_compressed_logs() {
        let dir = TempDir::new();
        let path = dir.0.join("a.log.gz");
        let mut encoder = GzEncoder::new(File::create(&path).unwrap(), Compression::default());
        encoder.write_all(PLAIN).unwrap();
        encoder.finish().unwrap();
        set_age(&path, days(10));

        let report = clean(
            &dir.0,
            Retention {
                max_age_days: 7,
                max_total_bytes: 0,
            },
            &HashSet::new(),
            SystemTime::now(),
        )
        .unwrap();

        assert_eq!(report.removed, 1);
        assert!(!path.exists());
    }

    #[test]
    fn trims_oldest_logs_to_the_size_limit() {
        let dir = TempDir::new();
        let oldest = dir.file("1.log", PLAIN, days(3));
        let middle = dir.file("2.log", PLAIN, days(2));
        let newest = dir.file("3.log", PLAIN, days(1));

        let report = clean(
            &dir.0,
            Retention {
                max_age_days: 0,
                max_total_bytes: (PLAIN.len() * 2) as u64,
            },
            &HashSet::new(),
            SystemTime::now(),
        )
        .unwrap();

        assert_eq!(report.removed, 1);
        assert!(!oldest.exists());
        assert!(middle.exists() && newest.exists());
    }

    #[test]
    fn keeps_files_in_use_and_recently_written() {
        let dir = TempDir::new();
        let open = dir.file("open.log", PLAIN, days(10));
        let recent = dir.file("recent.log", PLAIN, Duration::from_secs(10));

        let report = clean(
            &dir.0,
            Retention {
                max_age_days: 1,
                max_total_bytes: 1,
            },
            &HashSet::from([open.clone()]),
            SystemTime::now(),
        )
        .unwrap();

        assert_eq!(report.removed, 0);
        assert!(open.exists() && recent.exists());
    }

    #[test]
    fn does_nothing_when_off_or_missing() {
        let dir = TempDir::new();
        let path = dir.file("a.log", PLAIN, days(100));
        let off = clean(
            &dir.0,
            Retention::default(),
            &HashSet::new(),
            SystemTime::now(),
        )
        .unwrap();
        assert_eq!(off.removed, 0);
        assert!(path.exists());

        let missing = clean(
            &dir.0.join("missing"),
            Retention {
                max_age_days: 1,
                max_total_bytes: 0,
            },
            &HashSet::new(),
            SystemTime::now(),
        )
        .unwrap();
        assert_eq!(missing, Cleanup::default());
    }
}
