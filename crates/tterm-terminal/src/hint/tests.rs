//! Behaviour tests for URL and path recognition.
//!
//! The URL cases come from Alacritty's `positive_url_parsing_regex_test` and
//! `negative_url_parsing_regex_test`, plus the post-processing cases its
//! `HintPostProcessor` describes. The path cases are tTerm's rule, which the
//! module documents as its own because Alacritty has no bare-path matcher.

use crate::grid::CellSize;
use crate::grid::GridPoint;
use crate::grid::GridSize;
use crate::hint::Hint;
use crate::hint::HintKind;
use crate::hint::find_hints;
use crate::hint::hint_at;
use crate::snapshot::Snapshot;
use crate::terminal::Config;
use crate::terminal::Scroll;
use crate::terminal::Terminal;

fn terminal(columns: usize, lines: usize) -> Terminal {
    Terminal::new(
        Config::default(),
        GridSize::new(columns, lines),
        CellSize::new(8, 16),
    )
}

fn snapshot(text: &str, columns: usize, lines: usize) -> Snapshot {
    let terminal = terminal(columns, lines);
    terminal.write(text.as_bytes());
    terminal.snapshot()
}

fn hints(text: &str, columns: usize, lines: usize) -> Vec<Hint> {
    find_hints(&snapshot(text, columns, lines))
}

fn urls(text: &str) -> Vec<String> {
    hints(text, 80, 10)
        .into_iter()
        .filter(|hint| hint.kind == HintKind::Url)
        .map(|hint| hint.text)
        .collect()
}

fn paths(text: &str) -> Vec<String> {
    hints(text, 80, 10)
        .into_iter()
        .filter(|hint| hint.kind == HintKind::Path)
        .map(|hint| hint.text)
        .collect()
}

#[test]
fn alacritty_url_schemes_are_recognized() {
    for url in [
        "ipfs://example.org",
        "ipns://example.org",
        "magnet:?xt=urn:btih:L0UDHA5H12",
        "mailto:example@example.org",
        "gemini://gemini.example.org/",
        "gopher://gopher.example.org",
        "https://www.example.org",
        "http://example.org",
        "news:some.news.portal",
        "file:///C:/Windows/",
        "file:/home/user/whatever",
        "git://github.com/user/repo.git",
        "ssh:git@github.com:user/repo.git",
        "ftp://ftp.example.org",
    ] {
        assert_eq!(urls(url), vec![url.to_string()], "did not recognize {url}");
    }
}

#[test]
fn url_lookalikes_are_not_urls() {
    for text in [
        "http::trace::on_request::log_parameters",
        "http//www.example.org",
        "/user:example.org",
        "mailto: example@example.org",
        "http://<script>alert('xss')</script>",
        "mailto:",
    ] {
        assert!(
            urls(text).is_empty(),
            "recognized a URL in {text}: {:?}",
            urls(text)
        );
    }
}

#[test]
fn trailing_punctuation_and_brackets_are_trimmed() {
    assert_eq!(
        urls("see https://example.com/foo)."),
        ["https://example.com/foo"]
    );
    assert_eq!(
        urls("https://example.com/a(b)"),
        ["https://example.com/a(b)"]
    );
    assert_eq!(urls("(https://example.com/a)"), ["https://example.com/a"]);
    assert_eq!(
        urls("https://example.com/x, next"),
        ["https://example.com/x"]
    );
    assert_eq!(
        urls("https://example.com/foo-bar"),
        ["https://example.com/foo-bar"]
    );
}

#[test]
fn a_url_can_wrap_onto_the_next_row() {
    let url = "https://example.com/very/long/path";
    // Twenty columns put the first twenty characters on row 0.
    let hints = hints(url, 20, 4);
    assert_eq!(hints.len(), 1);
    let hint = &hints[0];
    assert_eq!(hint.kind, HintKind::Url);
    assert_eq!(hint.text, url);
    assert_eq!(hint.range.start, GridPoint::new(0, 0));
    assert_eq!(hint.range.end.line, 1);
    assert_eq!(
        hint_at(&snapshot(url, 20, 4), GridPoint::new(1, 2)),
        Some(hint.clone())
    );
}

#[test]
fn paths_are_recognized_conservatively() {
    for path in [
        "/usr/local/bin/tool",
        "~/src/main.rs",
        "./run.sh",
        "../lib",
        "C:\\Users\\me\\file.txt",
        "src/main.rs",
        "\\\\server\\share",
    ] {
        assert_eq!(
            paths(path),
            vec![path.to_string()],
            "did not recognize {path}"
        );
    }
    for text in ["and/or", "foo/bar", "https://example.com/a"] {
        assert!(
            paths(text).is_empty(),
            "recognized a path in {text}: {:?}",
            paths(text)
        );
    }
}

#[test]
fn path_punctuation_is_trimmed() {
    assert_eq!(paths("see /usr/bin."), ["/usr/bin"]);
    assert_eq!(paths("(/usr/bin)"), ["/usr/bin"]);
    assert_eq!(paths("\"~/src/main.rs\""), ["~/src/main.rs"]);
}

#[test]
fn a_path_can_wrap_onto_the_next_row() {
    let path = "/usr/local/bin/tool";
    let hints = hints(path, 12, 4);
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].kind, HintKind::Path);
    assert_eq!(hints[0].text, path);
    assert_eq!(hints[0].range.start.line, 0);
    assert_eq!(hints[0].range.end.line, 1);
}

#[test]
fn a_file_url_is_not_also_a_path() {
    let hints = hints("file:///home/user/file.txt", 80, 5);
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].kind, HintKind::Url);
}

#[test]
fn a_hint_that_ends_at_the_frame_edge_is_found() {
    // Ten columns and one row: the URL fills the whole frame, so its
    // exclusive end equals the number of characters and has no offsets
    // entry of its own.
    let url = hints("http://a.b", 10, 1);
    assert_eq!(url.len(), 1);
    assert_eq!(url[0].text, "http://a.b");
    let path = hints("/usr/bin/x", 10, 1);
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].kind, HintKind::Path);
}

#[test]
fn hint_at_uses_grid_coordinates() {
    let frame = snapshot("hello https://example.com/x world", 80, 5);
    let inside = GridPoint::new(0, 8);
    let outside = GridPoint::new(0, 0);
    assert!(hint_at(&frame, inside).is_some());
    assert_eq!(hint_at(&frame, outside), None);
}

#[test]
fn grid_line_inverts_screen_row() {
    let terminal = terminal(20, 3);
    terminal.write(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
    terminal.scroll(Scroll::Top);
    let frame = terminal.snapshot();
    assert!(frame.display_offset() > 0);
    for row in 0..frame.line_count() {
        let line = frame.grid_line(row).expect("row is visible");
        assert_eq!(frame.screen_row(line), Some(row));
    }
    assert_eq!(frame.grid_line(frame.line_count()), None);
}

#[test]
fn random_text_does_not_panic() {
    // A small deterministic generator keeps the test self-contained and
    // reproducible; the alphabet is chosen to hit every recognizer branch.
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let alphabet: Vec<char> = "abcXYZ019:/\\\\.-_()[]<>\"'~ há中🙂\t".chars().collect();
    for _ in 0..200 {
        let mut text = String::new();
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let length = (state >> 32) as usize % 80;
        for _ in 0..length {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(alphabet[(state >> 33) as usize % alphabet.len()]);
        }
        let frame = snapshot(&text, 24, 6);
        let _ = find_hints(&frame);
    }
}
