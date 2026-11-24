use super::*;
use flate2::Compression;
use flate2::write::GzEncoder;

/// The fixture is generated from the Tauri version's own serializer by
/// `src/components/TerminalTab/legacySnapshotFixture.test.ts`.
const FIXTURE: &str =
    include_str!("../../../../../docs/gpui-migration/reports/assets/P2-19/legacy.json");

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("tterm-legacy-scrollback-{}", uuid::Uuid::new_v4())))
    }

    fn create(&self) {
        fs::create_dir_all(&self.0).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_content() -> String {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    fixture["serialized"].as_str().unwrap().to_string()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn write_gzip(dir: &Path, tab_id: &str, bytes: &[u8]) -> Vec<u8> {
    fs::create_dir_all(dir).unwrap();
    let compressed = gzip(bytes);
    fs::write(dir.join(format!("{tab_id}.gz")), &compressed).unwrap();
    compressed
}

#[test]
fn the_tauri_fixture_round_trips_without_changing_its_file() {
    let dir = TempDir::new();
    let content = fixture_content();
    let compressed = write_gzip(&dir.0, "tab-1", content.as_bytes());

    let saved = read_legacy_snapshot(&dir.0, "tab-1").unwrap();

    assert_eq!(saved.tab_id, "tab-1");
    assert_eq!(saved.content, content);
    assert!(saved.saved_at > 0);
    assert!(saved.content.contains("scrolled-off-0"));
    assert!(saved.content.contains("\u{1b}[31mred\u{1b}[0m 中文"));
    assert!(saved.content.contains("$ cargo test"));
    assert!(!saved.content.contains("vim-only"));
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn reading_does_not_prune_or_delete_anything() {
    let dir = TempDir::new();
    write_snapshots(
        &dir.0,
        &[ScrollbackSnapshot {
            tab_id: "tab-1".to_string(),
            content: "one".to_string(),
        }],
    )
    .unwrap();
    fs::write(dir.0.join("tab-2.gz"), b"not gzip").unwrap();
    fs::write(dir.0.join("notes.txt"), b"keep").unwrap();

    assert_eq!(
        read_legacy_snapshot(&dir.0, "tab-1").unwrap().content,
        "one"
    );
    assert!(read_legacy_snapshot(&dir.0, "tab-2").is_none());

    assert!(dir.0.join("tab-1.gz").exists());
    assert_eq!(fs::read(dir.0.join("tab-2.gz")).unwrap(), b"not gzip");
    assert_eq!(fs::read(dir.0.join("notes.txt")).unwrap(), b"keep");
}

#[test]
fn tab_ids_that_could_leave_the_folder_are_refused() {
    let dir = TempDir::new();
    let compressed = write_gzip(&dir.0, "tab-1", b"keep");
    for tab_id in ["", "../tab-1", "a/b", "a\\b", "tab.1"] {
        assert!(read_legacy_snapshot(&dir.0, tab_id).is_none());
    }
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn a_missing_folder_or_file_has_no_snapshot() {
    let dir = TempDir::new();
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert!(!dir.0.exists());
    dir.create();
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
}

#[test]
fn a_corrupt_header_is_refused_and_kept() {
    let dir = TempDir::new();
    write_gzip(&dir.0, "tab-1", b"legacy");
    fs::write(dir.0.join("tab-1.gz"), b"not gzip").unwrap();
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), b"not gzip");
}

#[test]
fn a_trailer_whose_crc_or_size_does_not_match_is_refused() {
    let dir = TempDir::new();
    let compressed = write_gzip(&dir.0, "tab-1", b"legacy");
    for offset in [compressed.len() - 8, compressed.len() - 4] {
        let mut damaged = compressed.clone();
        damaged[offset] ^= 0xff;
        fs::write(dir.0.join("tab-1.gz"), &damaged).unwrap();
        assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
        assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), damaged);
    }
}

#[test]
fn a_truncated_trailer_is_refused() {
    let dir = TempDir::new();
    let mut compressed = gzip(b"legacy");
    compressed.truncate(compressed.len() - 1);
    dir.create();
    fs::write(dir.0.join("tab-1.gz"), &compressed).unwrap();
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn bytes_after_the_gzip_member_are_refused() {
    let dir = TempDir::new();
    let mut compressed = write_gzip(&dir.0, "tab-1", b"legacy");
    compressed.push(b'x');
    fs::write(dir.0.join("tab-1.gz"), &compressed).unwrap();
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn non_utf8_content_is_refused() {
    let dir = TempDir::new();
    let compressed = write_gzip(&dir.0, "tab-1", &[0xff, 0xfe]);
    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn a_file_larger_than_the_limit_is_refused_without_reading_it() {
    let dir = TempDir::new();
    dir.create();
    let path = dir.0.join("tab-1.gz");
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_SNAPSHOT_BYTES + 1).unwrap();
    drop(file);

    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::metadata(&path).unwrap().len(), MAX_SNAPSHOT_BYTES + 1);
}

#[test]
fn content_that_expands_past_the_limit_is_refused() {
    let dir = TempDir::new();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    let chunk = [b'a'; 8 * 1024];
    let mut written = 0_u64;
    while written <= MAX_SNAPSHOT_BYTES {
        encoder.write_all(&chunk).unwrap();
        written += u64::try_from(chunk.len()).unwrap();
    }
    let compressed = encoder.finish().unwrap();
    dir.create();
    fs::write(dir.0.join("tab-1.gz"), &compressed).unwrap();

    assert!(read_legacy_snapshot(&dir.0, "tab-1").is_none());
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}

#[test]
fn a_snapshot_with_its_documented_scrolled_line_and_color_is_read() {
    let dir = TempDir::new();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let expected: Vec<&str> = fixture["expectedVisible"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap())
        .collect();
    let compressed = write_gzip(&dir.0, "tab-1", fixture_content().as_bytes());

    let saved = read_legacy_snapshot(&dir.0, "tab-1").unwrap();

    let lines: Vec<&str> = saved.content.split("\r\n").collect();
    assert_eq!(lines.len(), expected.len());
    assert_eq!(lines.first(), Some(&"scrolled-off-0"));
    assert!(saved.content.contains("\u{1b}[31mred\u{1b}[0m 中文"));
    assert!(lines.contains(&"$ cargo test"));
    assert!(lines.contains(&"$ "));
    assert!(expected.contains(&"red 中文"));
    assert_eq!(fixture["expectedColor"]["foreground"], "index:1");
    assert_eq!(fs::read(dir.0.join("tab-1.gz")).unwrap(), compressed);
}
