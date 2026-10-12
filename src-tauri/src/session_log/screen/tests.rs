use super::*;

fn texts(lines: Vec<ScreenLine>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| match line {
            ScreenLine::Text { text, .. } => text,
            ScreenLine::Event { name, .. } => format!("<{name}>"),
        })
        .collect()
}

fn run(rows: u16, cols: u16, data: &[u8]) -> Vec<String> {
    let mut log = OutputLog::new(rows, cols);
    let mut lines = log.advance(data);
    lines.extend(log.finish());
    texts(lines)
}

#[test]
fn logs_lines_once_they_scroll_off_the_top() {
    let mut log = OutputLog::new(3, 20);
    assert!(log.advance(b"one\r\ntwo\r\nthree").is_empty());
    assert_eq!(texts(log.advance(b"\r\nfour")), vec!["one"]);
    assert_eq!(texts(log.finish()), vec!["two", "three", "four"]);
}

#[test]
fn keeps_only_the_last_frame_of_a_progress_line() {
    let lines = run(5, 40, b"get 10%\rget 50%\rget 100%\r\ndone\r\n");
    assert_eq!(lines, vec!["get 100%", "done"]);
}

#[test]
fn redraws_of_several_lines_leave_the_final_frame() {
    // Two progress rows redrawn in place by moving the cursor up.
    let data =
        b"a: 1\r\nb: 1\r\n\x1b[2Aa: 2\x1b[K\r\nb: 2\x1b[K\r\n\x1b[2Aa: 3\x1b[K\r\nb: 3\x1b[K\r\n";
    assert_eq!(run(10, 20, data), vec!["a: 3", "b: 3"]);
}

#[test]
fn rejoins_lines_the_terminal_wrapped() {
    let lines = run(3, 4, b"abcdefghij\r\nnext\r\n");
    assert_eq!(lines, vec!["abcdefghij", "next"]);
}

#[test]
fn wrapped_line_split_across_scroll_and_screen_stays_whole() {
    let mut log = OutputLog::new(2, 4);
    let mut lines = log.advance(b"abcdefgh\r\nxy");
    lines.extend(log.finish());
    assert_eq!(texts(lines), vec!["abcdefgh", "xy"]);
}

#[test]
fn full_screen_programs_leave_a_note_instead_of_frames() {
    let data = b"$ vim\r\n\x1b[?1049h\x1b[H\x1b[2Jfile contents\x1b[5;1H~\x1b[?1049l$ ls\r\n";
    assert_eq!(
        run(10, 20, data),
        vec!["<fullscreen_start>", "<fullscreen_end>", "$ vim", "$ ls"]
    );
}

#[test]
fn clearing_the_screen_keeps_what_it_showed() {
    let data = b"$ ls\r\na b c\r\n$ clear\r\n\x1b[H\x1b[2J\x1b[3J$ ";
    assert_eq!(run(10, 20, data), vec!["$ ls", "a b c", "$ clear", "$"]);
}

#[test]
fn lines_scrolled_out_of_a_partial_region_are_not_logged() {
    // A status line at the top with a scroll region below it.
    let data = b"status\x1b[2;3r\x1b[2;1Ha\r\nb\r\nc\r\nd";
    assert_eq!(run(3, 10, data), vec!["status", "c", "d"]);
}

#[test]
fn wide_characters_take_two_columns() {
    assert_eq!(run(3, 5, "中文ab\r\n".as_bytes()), vec!["中文ab"]);
    // A wide character that does not fit wraps whole.
    assert_eq!(run(3, 5, "abcd中\r\n".as_bytes()), vec!["abcd中"]);
    // Overwriting half of one leaves no stray half.
    assert_eq!(run(3, 10, "中\rx\r\n".as_bytes()), vec!["x"]);
}

#[test]
fn combining_marks_stay_with_their_character() {
    assert_eq!(run(3, 10, "e\u{301}x".as_bytes()), vec!["e\u{301}x"]);
}

#[test]
fn shrinking_pushes_top_rows_into_the_log() {
    let mut log = OutputLog::new(4, 10);
    assert!(log.advance(b"1\r\n2\r\n3\r\n4").is_empty());
    assert_eq!(texts(log.resize(2, 10)), vec!["1", "2"]);
    assert_eq!(texts(log.finish()), vec!["3", "4"]);
}

#[test]
fn prompt_redraw_after_backspaces_is_logged_once() {
    let mut log = OutputLog::new(5, 40);
    assert!(log
        .advance(b"\r\x1b[0m\x1b[32m> \x1b[36m~\x1b[0m \x1b[K")
        .is_empty());
    assert!(log.advance(b"l").is_empty());
    assert!(log.advance(b"\x08ll").is_empty());
    assert!(log
        .advance(b"\x08\x08\x1b[32ml\x1b[32ml\x1b[39m")
        .is_empty());
    assert!(log.advance(b"\x1b[?1l\x1b>\r\r\n").is_empty());
    assert_eq!(texts(log.finish()), vec!["> ~ ll"]);
}

#[test]
fn oldest_pending_tracks_rows_still_on_screen() {
    let mut log = OutputLog::new(2, 10);
    assert!(log.oldest_pending().is_none());
    log.advance(b"a");
    let first = log.oldest_pending().expect("row started");
    log.advance(b"\r\nb\r\nc");
    let after = log.oldest_pending().expect("rows on screen");
    assert!(after >= first);
    log.finish();
    assert!(log.oldest_pending().is_none());
}
