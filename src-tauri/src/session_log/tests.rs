use super::naming::{part_path, path_or_compressed_exists, render_name, sanitize_file_name};
use super::plain::{InputStream, PlainLog};
use super::rotating::compress_file;
use super::*;
use chrono::{DateTime, Local};
use std::fs::File;
use std::io::Read;

#[test]
fn sanitizes_cross_platform_file_names() {
    assert_eq!(sanitize_file_name("root@host:22/a*b?"), "root@host_22_a_b_");
    assert_eq!(sanitize_file_name("..."), "terminal-session");
}

#[test]
fn renders_session_name_tokens() {
    let metadata = SessionMetadata {
        tab_id: "tab:1".to_string(),
        session_nonce: 1,
        session_type: "ssh".to_string(),
        profile: "prod/api".to_string(),
        host: "10.0.0.1".to_string(),
        port: 22,
        username: "root".to_string(),
        started_at_ms: 0,
    };
    let rendered = render_name(
        "{profile}-{host}-{port}-{username}-{type}-{sessionId}",
        &metadata,
    );
    assert_eq!(rendered, "prod_api-10.0.0.1-22-root-ssh-tab_1");
}

#[test]
fn renders_folders_from_the_template() {
    let metadata = SessionMetadata {
        tab_id: "tab-1".to_string(),
        session_nonce: 1,
        session_type: "ssh".to_string(),
        profile: "prod/api".to_string(),
        host: "10.0.0.1".to_string(),
        port: 22,
        username: String::new(),
        started_at_ms: 0,
    };
    let year = Local::now().format("%Y").to_string();
    assert_eq!(
        render_name("{year}/{profile}/{host}-{sessionId}", &metadata),
        format!("{year}/prod_api/10.0.0.1-tab-1")
    );
    // Backslashes separate folders too; empty and dot-only folders are dropped.
    assert_eq!(
        render_name("/logs\\{username}//../{host}", &metadata),
        "logs/10.0.0.1"
    );
    assert_eq!(render_name("{type}/", &metadata), "ssh");
}

#[test]
fn limits_how_deep_the_template_goes() {
    let mut config = AppConfig {
        terminal_log_directory: std::env::temp_dir().to_string_lossy().into_owned(),
        ..AppConfig::default()
    };
    config.terminal_log_name_template = "a/b/c/d/e/{host}".to_string();
    assert_eq!(LogConfig::from_app_config(&config).err(), None);
    config.terminal_log_name_template = "a/b/c/d/e/f/{host}".to_string();
    assert!(LogConfig::from_app_config(&config).is_err());
}

#[test]
fn part_paths_follow_folders() {
    let directory = Path::new("/logs");
    assert_eq!(
        part_path(directory, "2026/prod-1", "tlog", 2),
        directory.join("2026").join("prod-1.002.tlog")
    );
}

#[test]
fn compression_preserves_content() {
    let directory = std::env::temp_dir().join(format!("tterm-log-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("session.log");
    fs::write(&source_path, b"terminal output").unwrap();
    compress_file(&source_path).unwrap();
    assert!(!source_path.exists());

    let mut decoder =
        flate2::read::GzDecoder::new(File::open(directory.join("session.log.gz")).unwrap());
    let mut content = String::new();
    decoder.read_to_string(&mut content).unwrap();
    assert_eq!(content, "terminal output");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn compressed_file_prevents_name_reuse() {
    let directory = std::env::temp_dir().join(format!("tterm-log-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.tlog");
    fs::write(directory.join("session.tlog.gz"), b"archive").unwrap();

    assert!(path_or_compressed_exists(&path));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn input_combines_keystrokes_into_submitted_commands() {
    let mut stream = InputStream::new();

    assert!(stream.advance(b"fast").is_empty());
    for byte in b"fetch" {
        assert!(stream.advance(&[*byte]).is_empty());
    }
    assert_eq!(stream.advance(b"\r"), vec!["fastfetch"]);
    assert!(stream.advance(b"\n").is_empty());
    assert_eq!(stream.advance(b"\x0c"), vec!["<Ctrl+L>"]);
    assert_eq!(stream.advance(b"\r"), vec!["<Enter>"]);
}

#[test]
fn input_follows_line_editing_keys() {
    let mut stream = InputStream::new();
    // Type "lss", move left, delete the extra "s", go home, insert nothing.
    assert!(stream.advance(b"lss\x1b[D\x1b[3~\x1b[H").is_empty());
    assert_eq!(stream.advance(b"\r"), vec!["ls"]);
    assert!(stream.advance(b"pwdx\x7f").is_empty());
    assert_eq!(stream.finish(), vec!["pwd"]);
}

/// The log lines without their timestamps.
fn untimed(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| match line.split_once("] ") {
            Some((_, rest)) => rest.trim_end().replacen("[", "", 1).replacen("]", "", 1),
            None => line.trim_end().to_string(),
        })
        .collect()
}

fn tick() -> DateTime<Local> {
    std::thread::sleep(Duration::from_millis(2));
    Local::now()
}

#[test]
fn plain_log_puts_typed_lines_between_the_output_around_them() {
    let mut log = PlainLog::new(10, 40, true);
    let mut lines = log.output(Local::now(), b"$ ");
    tick();
    lines.extend(log.input(tick(), b"ls\r"));
    // The prompt is still on screen, so the typed line waits for it.
    assert!(lines.is_empty());
    tick();
    lines.extend(log.output(Local::now(), b"ls\r\nfile\r\n$ "));
    lines.extend(log.finish(tick()));
    assert_eq!(
        untimed(lines),
        vec!["OUTPUT $ ls", "INPUT ls", "OUTPUT file", "OUTPUT $"]
    );
}

#[test]
fn plain_log_without_timestamps_keeps_output_bare() {
    let mut log = PlainLog::new(10, 40, false);
    let mut lines = log.event(tick(), "session_start", &serde_json::json!({}));
    lines.extend(log.output(Local::now(), b"$ "));
    lines.extend(log.input(tick(), b"ls\r"));
    tick();
    lines.extend(log.output(Local::now(), b"ls\r\nfile\r\n\r\n$ "));
    lines.extend(log.finish(tick()));
    assert_eq!(
        lines,
        vec![
            "[EVENT] session_start {}\n",
            "$ ls\n",
            "[INPUT] ls\n",
            "file\n",
            "\n",
            "$\n",
        ]
    );
}

#[test]
fn plain_log_writes_events_right_away_on_an_empty_screen() {
    let mut log = PlainLog::new(10, 40, true);
    let lines = log.event(Local::now(), "session_start", &serde_json::json!({}));
    assert_eq!(untimed(lines), vec!["EVENT session_start {}"]);
}

#[test]
fn plain_log_notes_full_screen_programs_in_order() {
    let mut log = PlainLog::new(10, 40, true);
    let mut lines = log.output(Local::now(), b"$ vim\r\n");
    tick();
    lines.extend(log.output(Local::now(), b"\x1b[?1049h\x1b[2Jtext\x1b[?1049l"));
    tick();
    lines.extend(log.output(Local::now(), b"$ ls\r\n"));
    lines.extend(log.finish(tick()));
    assert_eq!(
        untimed(lines),
        vec![
            "OUTPUT $ vim",
            "EVENT fullscreen_start {}",
            "EVENT fullscreen_end {}",
            "OUTPUT $ ls",
        ]
    );
}

#[test]
fn policy_and_manual_choice_decide_recording() {
    let config = |record_by_default| LogConfig {
        record_by_default,
        record_input: false,
        files: LogFiles {
            directory: PathBuf::new(),
            format: LOG_FORMAT_BOTH.to_string(),
            name_template: "x".to_string(),
            max_file_size_bytes: 1,
            compress: false,
            plain_timestamps: true,
        },
        retention: Retention::default(),
    };
    let off = config(false);
    let on = config(true);

    assert!(!off.records(SessionLogPolicy::Default, None));
    assert!(on.records(SessionLogPolicy::Default, None));
    assert!(off.records(SessionLogPolicy::Always, None));
    assert!(!on.records(SessionLogPolicy::Never, None));
    assert!(off.records(SessionLogPolicy::Never, Some(true)));
    assert!(!on.records(SessionLogPolicy::Always, Some(false)));
}

#[test]
fn policy_reads_profile_labels() {
    assert_eq!(
        SessionLogPolicy::from_label(Some("always")),
        SessionLogPolicy::Always
    );
    assert_eq!(
        SessionLogPolicy::from_label(Some("never")),
        SessionLogPolicy::Never
    );
    assert_eq!(
        SessionLogPolicy::from_label(None),
        SessionLogPolicy::Default
    );
    assert_eq!(
        SessionLogPolicy::from_label(Some("other")),
        SessionLogPolicy::Default
    );
}
