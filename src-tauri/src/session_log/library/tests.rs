use super::*;
use flate2::write::GzEncoder;
use flate2::Compression;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("tterm-log-library-{}", uuid::Uuid::new_v4()));
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
const PLAIN_HEAD: &str = "# tTerm session log\n# type=ssh profile=prod api host=10.0.0.8 port=22 username=root startedAt=1700000000000 tab=t\n";

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
        tab_id: "t".into(),
    };
    assert_eq!(read_metadata(&raw), Some(expected.clone()));
    assert_eq!(read_metadata(&plain), Some(expected.clone()));
    // Plain headers from before the tab was recorded.
    let old = dir.write(
        "c.log",
        "# tTerm session log\n# type=ssh profile=prod api host=10.0.0.8 port=22 username=root startedAt=1700000000000\n",
    );
    assert_eq!(
        read_metadata(&old),
        Some(LogMetadata {
            tab_id: String::new(),
            ..expected
        })
    );
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
fn lists_sessions_in_folders() {
    let dir = TempDir::new();
    fs::create_dir_all(dir.0.join("2026/10")).unwrap();
    fs::create_dir_all(dir.0.join(".hidden")).unwrap();
    dir.write("top.log", PLAIN_HEAD);
    dir.write("2026/10/s.tlog", &raw_log(&[data(1, b"hi")]));
    dir.write("2026/10/s.log", PLAIN_HEAD);
    dir.write(".hidden/h.log", PLAIN_HEAD);

    let sessions = list(&dir.0, &HashSet::new()).unwrap();
    let mut ids: Vec<&str> = sessions.iter().map(|session| session.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, ["2026/10/s", "top"]);
    let nested = sessions
        .iter()
        .find(|session| session.id == "2026/10/s")
        .unwrap();
    assert!(nested.has_raw && nested.has_plain);
    assert!(load_recording(&dir.0, "2026/10/s").is_ok());
}

#[test]
fn looks_only_as_deep_as_a_template_may_go() {
    let dir = TempDir::new();
    let deep = ["a", "b", "c", "d", "e"].join("/");
    let too_deep = format!("{deep}/f");
    fs::create_dir_all(dir.0.join(&too_deep)).unwrap();
    dir.write(&format!("{deep}/s.log"), PLAIN_HEAD);
    dir.write(&format!("{too_deep}/s.log"), PLAIN_HEAD);
    let sessions = list(&dir.0, &HashSet::new()).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, format!("{deep}/s"));
}

#[test]
fn deleting_removes_folders_left_empty() {
    let dir = TempDir::new();
    fs::create_dir_all(dir.0.join("2026/10")).unwrap();
    fs::create_dir_all(dir.0.join("2026/11")).unwrap();
    dir.write("2026/10/s.log", PLAIN_HEAD);
    dir.write("2026/11/t.log", PLAIN_HEAD);
    delete(&dir.0, "2026/10/s", &HashSet::new()).unwrap();
    assert!(!dir.0.join("2026/10").exists());
    assert!(dir.0.join("2026/11/t.log").exists());
    delete(&dir.0, "2026/11/t", &HashSet::new()).unwrap();
    assert!(!dir.0.join("2026").exists());
    assert!(dir.0.exists());
}

#[test]
fn deletes_many_and_reports_the_ones_it_could_not() {
    let dir = TempDir::new();
    dir.write("a.log", PLAIN_HEAD);
    let open = dir.write("b.log", PLAIN_HEAD);
    dir.write("c.log", PLAIN_HEAD);
    let report = delete_many(
        &dir.0,
        &["a".into(), "b".into(), "c".into(), "../x".into()],
        &HashSet::from([open.clone()]),
    );
    assert_eq!(report.deleted, ["a", "c"]);
    let failed: Vec<&str> = report
        .failed
        .iter()
        .map(|failure| failure.id.as_str())
        .collect();
    assert_eq!(failed, ["b", "../x"]);
    assert!(open.exists());
    assert!(!dir.0.join("a.log").exists() && !dir.0.join("c.log").exists());
}

#[test]
fn rejects_ids_that_are_not_sessions() {
    let dir = TempDir::new();
    dir.write("x.log", "not ours\n");
    assert!(files_of(&dir.0, "../etc").is_err());
    assert!(files_of(&dir.0, "a/../x").is_err());
    assert!(files_of(&dir.0, "/x").is_err());
    assert!(files_of(&dir.0, "a\\x").is_err());
    assert!(files_of(&dir.0, "C:/x").is_err());
    assert!(files_of(&dir.0, "x").is_err());
    assert!(files_of(&dir.0, "missing").is_err());
}
