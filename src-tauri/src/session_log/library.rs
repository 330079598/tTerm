//! The log directory as a list of sessions. A session is every file in one
//! folder that shares a base name: its raw (`.tlog`) and plain (`.log`) logs,
//! each split into parts once it reaches the size limit, any of them gzipped.
//! The name template may put logs in folders below the log directory; a
//! session's id is its folders and base name joined by `/`. Only files whose
//! header shows tTerm wrote them are listed or touched.

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
/// How many folders deep below the log directory logs are looked for; the
/// name template may not go deeper.
pub(super) const MAX_FOLDER_DEPTH: usize = 5;

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
    } else {
        (stem.strip_suffix(".log")?, Kind::Plain)
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
    /// The tab the session ran in; logs from before it was recorded in the
    /// plain header have none there.
    pub tab_id: String,
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

/// `# type=ssh profile=prod host=10.0.0.8 port=22 username=root startedAt=… tab=…`;
/// values may contain spaces, so each runs up to the next key.
fn parse_plain_metadata(line: &str) -> LogMetadata {
    const KEYS: [&str; 7] = [
        "type=",
        " profile=",
        " host=",
        " port=",
        " username=",
        " startedAt=",
        " tab=",
    ];
    let mut values = [""; 7];
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
        tab_id: values[6].to_string(),
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
    tab_id: String,
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

/// A file in the log directory or one of its folders.
pub(super) struct FoundFile {
    pub path: PathBuf,
    /// The folders between the log directory and the file, joined by `/`;
    /// empty for a file at the top.
    pub folder: String,
    pub name: String,
}

/// The files and the folders directly in `path`. Symbolic links, hidden
/// folders and names that are not UTF-8 are left out.
fn read_folder(path: &Path, folder: &str) -> std::io::Result<(Vec<FoundFile>, Vec<String>)> {
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in fs::read_dir(path)?.filter_map(Result::ok) {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            files.push(FoundFile {
                path: entry.path(),
                folder: folder.to_string(),
                name,
            });
        } else if file_type.is_dir() && !name.starts_with('.') {
            folders.push(name);
        }
    }
    Ok((files, folders))
}

/// Every file in `directory` and its folders, down to `MAX_FOLDER_DEPTH`
/// folders deep. A folder that cannot be read is skipped; only the log
/// directory itself must be readable, and a missing one has no files.
pub(super) fn walk(directory: &Path) -> Result<Vec<FoundFile>, String> {
    let (mut found, folders) = match read_folder(directory, "") {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "Failed to read terminal log directory '{}': {error}",
                directory.display()
            ))
        }
    };
    let mut pending: Vec<(String, usize)> = folders.into_iter().map(|name| (name, 1)).collect();
    while let Some((folder, depth)) = pending.pop() {
        let Ok((files, folders)) = read_folder(&join_relative(directory, &folder), &folder) else {
            continue;
        };
        found.extend(files);
        if depth < MAX_FOLDER_DEPTH {
            pending.extend(
                folders
                    .into_iter()
                    .map(|name| (format!("{folder}/{name}"), depth + 1)),
            );
        }
    }
    Ok(found)
}

/// `relative`'s `/`-separated parts joined onto `directory`.
pub(super) fn join_relative(directory: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(directory.to_path_buf(), |path, part| path.join(part))
}

/// Removes `from` and the folders above it that are left empty, up to but not
/// including `root`.
pub(super) fn remove_empty_folders(root: &Path, from: &Path) {
    let mut current = from;
    while current != root && current.starts_with(root) {
        if fs::remove_dir(current).is_err() {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
}

fn session_id(folder: &str, base: &str) -> String {
    if folder.is_empty() {
        base.to_string()
    } else {
        format!("{folder}/{base}")
    }
}

/// Groups log files into sessions by folder and base name, each session's
/// files ordered by kind then part.
fn group_sessions(found: Vec<FoundFile>) -> BTreeMap<String, Vec<LogFile>> {
    let mut sessions: BTreeMap<String, Vec<LogFile>> = BTreeMap::new();
    for file in found {
        let Some(parsed) = parse_file_name(&file.name) else {
            continue;
        };
        sessions
            .entry(session_id(&file.folder, &parsed.base))
            .or_default()
            .push(LogFile {
                path: file.path,
                kind: parsed.kind,
                part: parsed.part,
            });
    }
    for files in sessions.values_mut() {
        files.sort_by_key(|file| (file.kind, file.part));
    }
    sessions
}

pub(super) fn list(directory: &Path, in_use: &HashSet<PathBuf>) -> Result<Vec<LogSession>, String> {
    let mut sessions = Vec::new();
    for (id, files) in group_sessions(walk(directory)?) {
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
            tab_id: metadata.tab_id,
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

/// The folder and base name an id names, if it could name a session: parts
/// that stay inside the log directory and that tTerm could have written.
fn parse_id(id: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = id.split('/').collect();
    if parts.len() > MAX_FOLDER_DEPTH + 1
        || parts.iter().any(|part| {
            part.is_empty() || *part == "." || *part == ".." || part.contains(['\\', ':'])
        })
    {
        return None;
    }
    Some(match id.rsplit_once('/') {
        Some((folder, base)) => (folder, base),
        None => ("", id),
    })
}

/// The files of one session, after checking `id` names one.
fn files_of(directory: &Path, id: &str) -> Result<Vec<LogFile>, String> {
    let (folder, base) = parse_id(id).ok_or("Invalid terminal log")?;
    let path = if folder.is_empty() {
        directory.to_path_buf()
    } else {
        join_relative(directory, folder)
    };
    let found = read_folder(&path, folder)
        .map(|(files, _)| files)
        .unwrap_or_default();
    let files = group_sessions(found)
        .remove(&session_id(folder, base))
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

/// Deletes every file of a session that is not being written, and the
/// folders the session leaves empty.
pub(super) fn delete(directory: &Path, id: &str, in_use: &HashSet<PathBuf>) -> Result<(), String> {
    let files = files_of(directory, id)?;
    if files.iter().any(|file| in_use.contains(&file.path)) {
        return Err("This session is still being logged; stop logging it first".to_string());
    }
    for file in &files {
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
    if let Some(folder) = files.first().and_then(|file| file.path.parent()) {
        remove_empty_folders(directory, folder);
    }
    Ok(())
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    deleted: Vec<String>,
    failed: Vec<DeleteFailure>,
}

#[derive(Debug, Serialize)]
pub struct DeleteFailure {
    id: String,
    message: String,
}

/// Deletes each session as `delete` does; one that cannot be deleted does not
/// stop the rest.
pub(super) fn delete_many(
    directory: &Path,
    ids: &[String],
    in_use: &HashSet<PathBuf>,
) -> DeleteReport {
    let mut report = DeleteReport::default();
    for id in ids {
        match delete(directory, id, in_use) {
            Ok(()) => report.deleted.push(id.clone()),
            Err(message) => report.failed.push(DeleteFailure {
                id: id.clone(),
                message,
            }),
        }
    }
    report
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
mod tests;
