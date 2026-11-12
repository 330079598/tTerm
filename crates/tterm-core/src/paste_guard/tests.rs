use super::{
    PASTE_PREVIEW_MAX_LINE_LENGTH, PASTE_PREVIEW_MAX_LINES, paste_needs_confirmation,
    summarize_paste,
};

#[test]
fn asks_for_text_with_any_line_break() {
    assert!(paste_needs_confirmation("ls\npwd", true, false));
    assert!(paste_needs_confirmation("rm -rf build\r\n", true, false));
    assert!(paste_needs_confirmation("echo hi\r", true, false));
}

#[test]
fn lets_a_single_line_through() {
    assert!(!paste_needs_confirmation("git status", true, false));
}

#[test]
fn trusts_bracketed_paste_mode_and_the_setting() {
    assert!(!paste_needs_confirmation("ls\npwd", true, true));
    assert!(!paste_needs_confirmation("ls\npwd", false, false));
}

#[test]
fn counts_lines_across_line_ending_styles_and_ignores_a_trailing_break() {
    let summary = summarize_paste("a\r\nb\rc\n");
    assert_eq!(summary.line_count, 3);
    assert_eq!(summary.preview, "a\nb\nc");

    let summary = summarize_paste("one\n");
    assert_eq!(summary.line_count, 1);
    assert_eq!(summary.preview, "one");

    let summary = summarize_paste("");
    assert_eq!(summary.line_count, 1);
    assert_eq!(summary.preview, "");

    let summary = summarize_paste("😀中文\n第二行");
    assert_eq!(summary.line_count, 2);
    assert_eq!(summary.preview, "😀中文\n第二行");
}

#[test]
fn shortens_long_pastes_and_long_lines() {
    let lines: Vec<String> = (0..PASTE_PREVIEW_MAX_LINES + 4)
        .map(|index| format!("line {index}"))
        .collect();
    let summary = summarize_paste(&lines.join("\n"));
    assert_eq!(summary.line_count, PASTE_PREVIEW_MAX_LINES + 4);
    assert_eq!(
        summary.preview.split('\n').count(),
        PASTE_PREVIEW_MAX_LINES + 1
    );
    assert!(summary.preview.ends_with("\n…"));

    let long = "x".repeat(PASTE_PREVIEW_MAX_LINE_LENGTH + 10);
    assert_eq!(
        summarize_paste(&format!("{long}\nok")).preview,
        format!("{}…\nok", "x".repeat(PASTE_PREVIEW_MAX_LINE_LENGTH))
    );

    let emoji = "😀".repeat(81);
    assert_eq!(
        summarize_paste(&emoji).preview,
        format!("{}…", "😀".repeat(80))
    );

    let combining = "e\u{301}".repeat(161);
    assert_eq!(
        summarize_paste(&combining).preview,
        format!("{}…", "e\u{301}".repeat(80))
    );
}
