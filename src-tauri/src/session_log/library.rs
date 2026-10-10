//! The log directory as a list of sessions. A session is every file that
//! shares a base name: its raw (`.tlog`) and plain (`.log`) logs, each split
//! into parts once it reaches the size limit, any of them gzipped. Only files
//! whose header shows tTerm wrote them are listed or touched.

use base64::Engine;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub(super) const PLAIN_HEADER: &str = "# tTerm session log";
const MAX_HEADER_BYTES: u64 = 64 * 1024;
/// Output a replay may load at once; longer recordings are left to export.
const MAX_REPLAY_BYTES: usize = 256 * 1024 * 1024;
/// Plain text the viewer may load at once.
const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_SIZE: (u16, u16) = (80, 24);

const FRAME_OUTPUT: u8 = 0;
const FRAME_RESIZE: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Raw,
    Plain,
}

#[derive(Debug, PartialEq, Eq)]
struct LogFileName {
    base: String,
    kind: Kind,
    part: u32,
}

/// `name.tlog`, `name.001.log`, `name.log.gz` and so on.
fn parse_file_name(name: &str) -> Option<LogFileName> {
    let stem = name.strip_suffix(".gz").unwrap_or(name);
    let (stem, kind) = if let Some(stem) = stem.strip_suffix(".tlog") {
        (stem, Kind::Raw)
    } else if let Some(stem) = stem.strip_suffix(".log") {
        (stem, Kind::Plain)
    } else {
        return None;
    };
    let (base, part) = match stem.rsplit_once('.') {
        Some((base, part)) if part.len() >= 3 && part.bytes().all(|byte| byte.is_ascii_digit()) => {
            (base, part.parse().ok()?)
        }
        _ => (stem, 0),
    };
    (!base.is_empty()).then(|| LogFileName {
        base: base.to_string(),
        kind,
        part,
    })
}

fn open_log(path: &Path) -> Result<Box<dyn Read>, String> {
    let file = File::open(path)
        .map_err(|err| format!("Failed to open terminal log '{}': {err}", path.display()))?;
    let compressed = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".gz"));
    Ok(if compressed {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    })
}

/// Who and what a log recorded, from its header.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct LogMetadata {
    pub session_type: String,
    pub profile: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub started_at_ms: u64,
}

#[derive(Deserialize)]
struct RawHeader {
    #[serde(rename = "type")]
    kind: String,
    metadata: RawHeaderMetadata,
}

#[derive(Deserialize)]
struct RawHeaderMetadata {
    /// Required: only tTerm's headers carry it.
    #[serde(rename = "sessionNonce")]
    _session_nonce: u32,
    #[serde(flatten)]
    metadata: LogMetadata,
}

/// The header of a tTerm log, or `None` for any other file.
pub(super) fn read_metadata(path: &Path) -> Option<LogMetadata> {
    let reader = BufReader::new(open_log(path).ok()?.take(MAX_HEADER_BYTES));
    let mut lines = reader.lines();
    let first = lines.next()?.ok()?;
    if first.starts_with(PLAIN_HEADER) {
        let second = lines.next().and_then(Result::ok).unwrap_or_default();
        return Some(parse_plain_metadata(&second));
    }
    let header: RawHeader = serde_json::from_str(&first).ok()?;
    (header.kind == "header").then_some(header.metadata.metadata)
}

/// `# type=ssh profile=prod host=10.0.0.8 port=22 username=root startedAt=…`;
/// values may contain spaces, so each runs up to the next key.
fn parse_plain_metadata(line: &str) -> LogMetadata {
    const KEYS: [&str; 6] = [
        "type=",
        " profile=",
        " host=",
        " port=",
        " username=",
        " startedAt=",
    ];
    let mut values = [""; 6];
    let Some(mut rest) = line.strip_prefix("# ") else {
        return LogMetadata::default();
    };
    for (index, key) in KEYS.iter().enumerate() {
        let Some(after) = rest.strip_prefix(key) else {
            break;
        };
        let end = KEYS
            .get(index + 1)
            .and_then(|next| after.find(next))
            .unwrap_or(after.len());
        values[index] = &after[..end];
        rest = &after[end..];
    }
    LogMetadata {
        session_type: values[0].to_string(),
        profile: values[1].to_string(),
        host: values[2].to_string(),
        port: values[3].parse().unwrap_or(0),
        username: values[4].to_string(),
        started_at_ms: values[5].parse().unwrap_or(0),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSession {
    id: String,
    session_type: String,
    profile: String,
    host: String,
    port: u16,
    username: String,
    started_at_ms: u64,
    modified_at_ms: u64,
    raw_bytes: u64,
    plain_bytes: u64,
    has_raw: bool,
    has_plain: bool,
    recording: bool,
}

struct LogFile {
    path: PathBuf,
    kind: Kind,
    part: u32,
}

/// The files of each session in `directory`, ordered by kind then part.
fn session_files(directory: &Path) -> Result<BTreeMap<String, Vec<LogFile>>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(format!(
                "Failed to read terminal log directory '{}': {error}",
                directory.display()
            ))
        }
    };
    let mut sessions: BTreeMap<String, Vec<LogFile>> = BTreeMap::new();
    for entry in entries.filter_map(Result::ok) {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some(parsed) = parse_file_name(&name) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        sessions.entry(parsed.base).or_default().push(LogFile {
            path: entry.path(),
            kind: parsed.kind,
            part: parsed.part,
        });
    }
    for files in sessions.values_mut() {
        files.sort_by_key(|file| (file.kind, file.part));
    }
    Ok(sessions)
}

pub(super) fn list(directory: &Path, in_use: &HashSet<PathBuf>) -> Result<Vec<LogSession>, String> {
    let mut sessions = Vec::new();
    for (id, files) in session_files(directory)? {
        let Some(metadata) = files.iter().find_map(|file| read_metadata(&file.path)) else {
            continue;
        };
        let mut session = LogSession {
            id,
            session_type: metadata.session_type,
            profile: metadata.profile,
            host: metadata.host,
            port: metadata.port,
            username: metadata.username,
            started_at_ms: metadata.started_at_ms,
            modified_at_ms: 0,
            raw_bytes: 0,
            plain_bytes: 0,
            has_raw: false,
            has_plain: false,
            recording: false,
        };
        for file in &files {
            let Ok(file_metadata) = fs::metadata(&file.path) else {
                continue;
            };
            let modified_ms = file_metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_millis() as u64);
            session.modified_at_ms = session.modified_at_ms.max(modified_ms);
            session.recording |= in_use.contains(&file.path);
            match file.kind {
                Kind::Raw => {
                    session.has_raw = true;
                    session.raw_bytes += file_metadata.len();
                }
                Kind::Plain => {
                    session.has_plain = true;
                    session.plain_bytes += file_metadata.len();
                }
            }
        }
        sessions.push(session);
    }
    sessions.sort_by(|a, b| {
        b.started_at_ms
            .cmp(&a.started_at_ms)
            .then_with(|| b.modified_at_ms.cmp(&a.modified_at_ms))
    });
    Ok(sessions)
}

/// The files of one session, after checking `id` names one.
fn files_of(directory: &Path, id: &str) -> Result<Vec<LogFile>, String> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return Err("Invalid terminal log".to_string());
    }
    let files = session_files(directory)?
        .remove(id)
        .filter(|files| files.iter().any(|file| read_metadata(&file.path).is_some()))
        .ok_or("The terminal log no longer exists")?;
    Ok(files)
}

fn files_of_kind(files: Vec<LogFile>, kind: Kind) -> Vec<LogFile> {
    files.into_iter().filter(|file| file.kind == kind).collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Frame {
    Output { at_us: u64, data: Vec<u8> },
    Resize { at_us: u64, cols: u16, rows: u16 },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    elapsed_us: u64,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    detail: Option<serde_json::Value>,
}

fn size_of(detail: &serde_json::Value) -> Option<(u16, u16)> {
    let cols = u16::try_from(detail.get("cols")?.as_u64()?).ok()?;
    let rows = u16::try_from(detail.get("rows")?.as_u64()?).ok()?;
    (cols > 0 && rows > 0).then_some((cols, rows))
}

/// Output and size changes of a session's raw log, in order, until `each`
/// breaks. Lines that do not parse (a log cut off mid-line) are skipped.
fn read_frames(
    files: &[LogFile],
    mut each: impl FnMut(Frame) -> Result<ControlFlow<()>, String>,
) -> Result<(), String> {
    for file in files {
        let reader = BufReader::new(open_log(&file.path)?);
        for line in reader.lines() {
            let line = line.map_err(|err| {
                format!(
                    "Failed to read terminal log '{}': {err}",
                    file.path.display()
                )
            })?;
            let Ok(event) = serde_json::from_str::<RawEvent>(&line) else {
                continue;
            };
            match event.kind.as_str() {
                "data" if event.direction.as_deref() == Some("output") => {
                    let Some(data) = event.data.and_then(|data| {
                        base64::engine::general_purpose::STANDARD.decode(data).ok()
                    }) else {
                        continue;
                    };
                    if each(Frame::Output {
                        at_us: event.elapsed_us,
                        data,
                    })?
                    .is_break()
                    {
                        return Ok(());
                    }
                }
                "header" | "data" => {}
                _ => {
                    if let Some((cols, rows)) = event.detail.as_ref().and_then(size_of) {
                        if each(Frame::Resize {
                            at_us: event.elapsed_us,
                            cols,
                            rows,
                        })?
                        .is_break()
                        {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A session's raw log as frames for the replay player: each is a kind byte
/// and the microseconds since the log started (u64), then for output a
/// length (u32) and the bytes, for a size change columns and rows (u16 each).
/// All little-endian.
pub(super) fn load_recording(directory: &Path, id: &str) -> Result<Vec<u8>, String> {
    let files = files_of_kind(files_of(directory, id)?, Kind::Raw);
    if files.is_empty() {
        return Err("This session has no raw log to replay".to_string());
    }
    let mut frames = Vec::new();
    let mut output_bytes = 0usize;
    read_frames(&files, |frame| {
        match frame {
            Frame::Output { at_us, data } => {
                output_bytes += data.len();
                if output_bytes > MAX_REPLAY_BYTES {
                    return Err(
                        "This recording is too large to replay here; export it instead".to_string(),
                    );
                }
                frames.push(FRAME_OUTPUT);
                frames.extend_from_slice(&at_us.to_le_bytes());
                frames.extend_from_slice(&(data.len() as u32).to_le_bytes());
                frames.extend_from_slice(&data);
            }
            Frame::Resize { at_us, cols, rows } => {
                frames.push(FRAME_RESIZE);
                frames.extend_from_slice(&at_us.to_le_bytes());
                frames.extend_from_slice(&cols.to_le_bytes());
                frames.extend_from_slice(&rows.to_le_bytes());
            }
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(frames)
}

/// A session's plain-text log, its parts joined without their repeated
/// headers.
pub(super) fn load_text(directory: &Path, id: &str) -> Result<Vec<u8>, String> {
    let files = files_of_kind(files_of(directory, id)?, Kind::Plain);
    if files.is_empty() {
        return Err("This session has no plain-text log".to_string());
    }
    let mut text = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let mut reader = BufReader::new(open_log(&file.path)?);
        if index > 0 {
            // Every part repeats the two header lines.
            let mut skipped = String::new();
            for _ in 0..2 {
                skipped.clear();
                reader
                    .read_line(&mut skipped)
                    .map_err(|err| err.to_string())?;
            }
        }
        let remaining = MAX_TEXT_BYTES + 1 - text.len() as u64;
        reader
            .take(remaining)
            .read_to_end(&mut text)
            .map_err(|err| {
                format!(
                    "Failed to read terminal log '{}': {err}",
                    file.path.display()
                )
            })?;
        if text.len() as u64 > MAX_TEXT_BYTES {
            return Err("This log is too large to show here; open it in a text editor".to_string());
        }
    }
    Ok(text)
}

/// Turns output bytes into text as they arrive, keeping a character split
/// between two chunks whole.
#[derive(Default)]
struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    fn push(&mut self, data: &[u8]) -> String {
        self.pending.extend_from_slice(data);
        let mut text = String::new();
        let mut start = 0;
        loop {
            match std::str::from_utf8(&self.pending[start..]) {
                Ok(valid) => {
                    text.push_str(valid);
                    start = self.pending.len();
                    break;
                }
                Err(error) => {
                    let valid_end = start + error.valid_up_to();
                    text.push_str(
                        std::str::from_utf8(&self.pending[start..valid_end]).expect("valid prefix"),
                    );
                    match error.error_len() {
                        Some(len) => {
                            text.push(char::REPLACEMENT_CHARACTER);
                            start = valid_end + len;
                        }
                        None => {
                            start = valid_end;
                            break;
                        }
                    }
                }
            }
        }
        self.pending.drain(..start);
        text
    }

    fn finish(&mut self) -> String {
        let text = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        text
    }
}

/// Writes a session's raw log as an asciicast v2 recording (asciinema).
pub(super) fn export_asciicast(directory: &Path, id: &str, target: &Path) -> Result<(), String> {
    let files = files_of(directory, id)?;
    let metadata = files
        .iter()
        .find_map(|file| read_metadata(&file.path))
        .unwrap_or_default();
    let files = files_of_kind(files, Kind::Raw);
    if files.is_empty() {
        return Err("This session has no raw log to export".to_string());
    }

    // The first known size is the recording's size.
    let mut size = None;
    read_frames(&files, |frame| {
        Ok(match frame {
            Frame::Resize { cols, rows, .. } => {
                size = Some((cols, rows));
                ControlFlow::Break(())
            }
            Frame::Output { .. } => ControlFlow::Continue(()),
        })
    })?;
    let (width, height) = size.unwrap_or(DEFAULT_SIZE);

    let title = match (metadata.profile.as_str(), metadata.host.as_str()) {
        _ if metadata.session_type != "ssh" => String::new(),
        ("", host) => host.to_string(),
        (profile, host) if host.is_empty() || profile == host => profile.to_string(),
        (profile, host) => format!("{profile} ({host})"),
    };
    let mut header = serde_json::json!({
        "version": 2,
        "width": width,
        "height": height,
        "env": { "TERM": "xterm-256color" },
    });
    if metadata.started_at_ms > 0 {
        header["timestamp"] = (metadata.started_at_ms / 1000).into();
    }
    if !title.is_empty() {
        header["title"] = title.into();
    }

    let file = File::create(target)
        .map_err(|err| format!("Failed to create '{}': {err}", target.display()))?;
    let mut out = BufWriter::new(file);
    let write_err = |err: std::io::Error| format!("Failed to write '{}': {err}", target.display());
    writeln!(out, "{header}").map_err(write_err)?;

    let mut text = Utf8Stream::default();
    let mut last_at = 0u64;
    let mut seen_size = false;
    read_frames(&files, |frame| {
        let event = match frame {
            Frame::Output { at_us, data } => {
                last_at = at_us;
                let chunk = text.push(&data);
                if chunk.is_empty() {
                    return Ok(ControlFlow::Continue(()));
                }
                serde_json::json!([seconds(at_us), "o", chunk])
            }
            Frame::Resize { at_us, cols, rows } => {
                // The header already has the first size.
                if !seen_size {
                    seen_size = true;
                    return Ok(ControlFlow::Continue(()));
                }
                serde_json::json!([seconds(at_us), "r", format!("{cols}x{rows}")])
            }
        };
        writeln!(out, "{event}").map_err(write_err)?;
        Ok(ControlFlow::Continue(()))
    })?;
    let rest = text.finish();
    if !rest.is_empty() {
        writeln!(out, "{}", serde_json::json!([seconds(last_at), "o", rest])).map_err(write_err)?;
    }
    out.flush().map_err(write_err)
}

fn seconds(at_us: u64) -> f64 {
    at_us as f64 / 1_000_000.0
}

/// Deletes every file of a session that is not being written.
pub(super) fn delete(directory: &Path, id: &str, in_use: &HashSet<PathBuf>) -> Result<(), String> {
    let files = files_of(directory, id)?;
    if files.iter().any(|file| in_use.contains(&file.path)) {
        return Err("This session is still being logged; stop logging it first".to_string());
    }
    for file in files {
        match fs::remove_file(&file.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to delete terminal log '{}': {error}",
                    file.path.display()
                ))
            }
        }
    }
    Ok(())
}

/// The file to show in the file manager for a session.
pub(super) fn first_file(directory: &Path, id: &str) -> Result<PathBuf, String> {
    files_of(directory, id)?
        .into_iter()
        .next()
        .map(|file| file.path)
        .ok_or_else(|| "The terminal log no longer exists".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("tterm-log-library-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, content: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, content).unwrap();
            path
        }

        fn write_gz(&self, name: &str, content: &str) -> PathBuf {
            let path = self.0.join(name);
            let mut encoder = GzEncoder::new(File::create(&path).unwrap(), Compression::default());
            encoder.write_all(content.as_bytes()).unwrap();
            encoder.finish().unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const RAW_HEADER: &str = r#"{"metadata":{"host":"10.0.0.8","port":22,"profile":"prod api","sessionNonce":1,"sessionType":"ssh","startedAtMs":1700000000000,"tabId":"t","username":"root"},"type":"header","version":1}"#;
    const PLAIN_HEAD: &str = "# tTerm session log\n# type=ssh profile=prod api host=10.0.0.8 port=22 username=root startedAt=1700000000000\n";

    fn data(at_us: u64, bytes: &[u8]) -> String {
        format!(
            r#"{{"type":"data","sequence":1,"timestampMs":0,"elapsedUs":{at_us},"direction":"output","encoding":"base64","data":"{}","crc32":0}}"#,
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    fn event(kind: &str, at_us: u64, detail: &str) -> String {
        format!(
            r#"{{"type":"{kind}","sequence":1,"timestampMs":0,"elapsedUs":{at_us},"detail":{detail}}}"#
        )
    }

    fn raw_log(lines: &[String]) -> String {
        let mut text = format!("{RAW_HEADER}\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    #[test]
    fn parses_log_file_names() {
        assert_eq!(
            parse_file_name("prod-1.tlog"),
            Some(LogFileName {
                base: "prod-1".into(),
                kind: Kind::Raw,
                part: 0
            })
        );
        assert_eq!(
            parse_file_name("a.b.002.log.gz"),
            Some(LogFileName {
                base: "a.b".into(),
                kind: Kind::Plain,
                part: 2
            })
        );
        assert_eq!(parse_file_name("x.12.log").unwrap().base, "x.12");
        assert_eq!(parse_file_name("notes.txt"), None);
        assert_eq!(parse_file_name(".log"), None);
    }

    #[test]
    fn reads_metadata_from_either_header() {
        let dir = TempDir::new();
        let raw = dir.write("a.tlog", &raw_log(&[]));
        let plain = dir.write("a.log", PLAIN_HEAD);
        let foreign = dir.write("b.log", "kernel: boot\n");
        let expected = LogMetadata {
            session_type: "ssh".into(),
            profile: "prod api".into(),
            host: "10.0.0.8".into(),
            port: 22,
            username: "root".into(),
            started_at_ms: 1_700_000_000_000,
        };
        assert_eq!(read_metadata(&raw), Some(expected.clone()));
        assert_eq!(read_metadata(&plain), Some(expected));
        assert_eq!(read_metadata(&foreign), None);
    }

    #[test]
    fn lists_sessions_with_their_parts() {
        let dir = TempDir::new();
        let open = dir.write("s1.tlog", &raw_log(&[data(1, b"hi")]));
        dir.write_gz("s1.001.tlog.gz", &raw_log(&[data(2, b"there")]));
        dir.write("s1.log", PLAIN_HEAD);
        dir.write("s2.log", PLAIN_HEAD);
        dir.write("other.log", "not ours\n");

        let sessions = list(&dir.0, &HashSet::from([open])).unwrap();
        assert_eq!(sessions.len(), 2);
        let s1 = sessions.iter().find(|session| session.id == "s1").unwrap();
        assert!(s1.has_raw && s1.has_plain && s1.recording);
        assert_eq!(s1.profile, "prod api");
        let s2 = sessions.iter().find(|session| session.id == "s2").unwrap();
        assert!(!s2.has_raw && s2.has_plain && !s2.recording);
    }

    #[test]
    fn replay_frames_follow_the_parts_in_order() {
        let dir = TempDir::new();
        dir.write(
            "s.tlog",
            &raw_log(&[
                event("session_start", 0, r#"{"rows":24,"cols":80}"#),
                data(10, b"one"),
            ]),
        );
        dir.write_gz(
            "s.001.tlog.gz",
            &raw_log(&[
                event("resize", 20, r#"{"rows":30,"cols":100}"#),
                data(30, b"two"),
                "{\"type\":\"data\",\"trunc".to_string(),
            ]),
        );
        let files = files_of_kind(files_of(&dir.0, "s").unwrap(), Kind::Raw);
        let mut frames = Vec::new();
        read_frames(&files, |frame| {
            frames.push(frame);
            Ok(ControlFlow::Continue(()))
        })
        .unwrap();
        assert_eq!(
            frames,
            vec![
                Frame::Resize {
                    at_us: 0,
                    cols: 80,
                    rows: 24
                },
                Frame::Output {
                    at_us: 10,
                    data: b"one".to_vec()
                },
                Frame::Resize {
                    at_us: 20,
                    cols: 100,
                    rows: 30
                },
                Frame::Output {
                    at_us: 30,
                    data: b"two".to_vec()
                },
            ]
        );

        let encoded = load_recording(&dir.0, "s").unwrap();
        assert_eq!(encoded[0], FRAME_RESIZE);
        assert_eq!(encoded.len(), (1 + 8 + 4) * 2 + (1 + 8 + 4 + 3) * 2);
    }

    #[test]
    fn plain_text_joins_parts_without_repeated_headers() {
        let dir = TempDir::new();
        dir.write("s.log", &format!("{PLAIN_HEAD}line 1\n"));
        dir.write_gz("s.001.log.gz", &format!("{PLAIN_HEAD}line 2\n"));
        let text = String::from_utf8(load_text(&dir.0, "s").unwrap()).unwrap();
        assert_eq!(text, format!("{PLAIN_HEAD}line 1\nline 2\n"));
    }

    #[test]
    fn exports_asciicast_v2() {
        let dir = TempDir::new();
        // "中" split across two chunks.
        dir.write(
            "s.tlog",
            &raw_log(&[
                event("session_start", 0, r#"{"rows":24,"cols":80}"#),
                data(500_000, b"a\xe4\xb8"),
                data(1_250_000, b"\xadb"),
                event("resize", 2_000_000, r#"{"rows":30,"cols":100}"#),
            ]),
        );
        let target = dir.0.join("out.cast");
        export_asciicast(&dir.0, "s", &target).unwrap();
        let cast = fs::read_to_string(&target).unwrap();
        let lines: Vec<serde_json::Value> = cast
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines[0]["version"], 2);
        assert_eq!(lines[0]["width"], 80);
        assert_eq!(lines[0]["height"], 24);
        assert_eq!(lines[0]["title"], "prod api (10.0.0.8)");
        assert_eq!(lines[0]["timestamp"], 1_700_000_000);
        assert_eq!(lines[1], serde_json::json!([0.5, "o", "a"]));
        assert_eq!(lines[2], serde_json::json!([1.25, "o", "中b"]));
        assert_eq!(lines[3], serde_json::json!([2.0, "r", "100x30"]));
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn deletes_a_session_unless_it_is_being_logged() {
        let dir = TempDir::new();
        let raw = dir.write("s.tlog", &raw_log(&[]));
        let plain = dir.write("s.log", PLAIN_HEAD);
        assert!(delete(&dir.0, "s", &HashSet::from([raw.clone()])).is_err());
        assert!(raw.exists());
        delete(&dir.0, "s", &HashSet::new()).unwrap();
        assert!(!raw.exists() && !plain.exists());
    }

    #[test]
    fn rejects_ids_that_are_not_sessions() {
        let dir = TempDir::new();
        dir.write("x.log", "not ours\n");
        assert!(files_of(&dir.0, "../etc").is_err());
        assert!(files_of(&dir.0, "x").is_err());
        assert!(files_of(&dir.0, "missing").is_err());
    }
}
