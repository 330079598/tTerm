use super::{SessionReport, SessionReportScanner};

fn scan(input: &[u8]) -> Vec<SessionReport> {
    let mut scanner = SessionReportScanner::new();
    scanner.scan(input)
}

fn scan_in_pieces(input: &[u8], pieces: &[usize]) -> Vec<SessionReport> {
    let mut scanner = SessionReportScanner::new();
    let mut reports = Vec::new();
    let mut start = 0;
    for end in pieces.iter().copied().chain(std::iter::once(input.len())) {
        reports.extend(scanner.scan(&input[start..end]));
        start = end;
    }
    reports
}

#[test]
fn complete_reports_are_returned_in_output_order() {
    let input = concat!(
        "before",
        "\x1b]7;file:///work/one\x07",
        "\x1b]133;A\x07",
        "\x1b]777;tterm-agent;codex;processing;abc-123;broadcast;C%3A%5Cwork\x07",
    );
    assert_eq!(
        scan(input.as_bytes()),
        vec![
            SessionReport::Cwd("/work/one".to_string()),
            SessionReport::Prompt,
            SessionReport::AgentReport(
                "tterm-agent;codex;processing;abc-123;broadcast;C%3A%5Cwork".to_string()
            ),
        ]
    );
}

#[test]
fn a_report_split_at_every_byte_is_still_one_report() {
    let input = b"\x1b]777;tterm-agent;claude;done;sess-1\x1b\\";
    for split in 1..input.len() {
        assert_eq!(
            scan_in_pieces(input, &[split]),
            vec![SessionReport::AgentReport(
                "tterm-agent;claude;done;sess-1".to_string()
            )],
            "split at {split}"
        );
    }
}

#[test]
fn prompt_marks_are_recognized_but_output_and_done_marks_are_not() {
    assert_eq!(scan(b"\x1b]133;A\x07"), vec![SessionReport::Prompt]);
    assert_eq!(scan(b"\x1b]633;A;aid=1\x1b\\"), vec![SessionReport::Prompt]);
    assert_eq!(scan(b"\x1b]133;C\x07"), vec![SessionReport::CommandStarted]);
    assert!(scan(b"\x1b]133;D;1\x07").is_empty());
}

#[test]
fn cwd_reports_keep_both_shell_integration_formats() {
    assert_eq!(
        scan(b"\x1b]7;file://host/work%20dir\x07"),
        Vec::<SessionReport>::new()
    );
    assert_eq!(
        scan(b"\x1b]7;file:///work%20dir\x07"),
        vec![SessionReport::Cwd("/work dir".to_string())]
    );
    assert_eq!(
        scan(b"\x1b]9;9;\"C:\\work dir\"\x07"),
        vec![SessionReport::Cwd("C:\\work dir".to_string())]
    );
    assert!(scan(b"\x1b]9;notification\x07").is_empty());
}

#[test]
fn other_777_payloads_and_malformed_reports_are_ignored() {
    assert!(scan(b"\x1b]777;notify;Title;Body\x07").is_empty());
    assert!(scan(b"\x1b]777;tterm-agent;\xff\x07").is_empty());
    assert!(scan(b"\x1b]9999;future\x07").is_empty());
}

#[test]
fn an_overlong_payload_is_dropped_whole() {
    let mut input = b"\x1b]777;tterm-agent;codex;done;sess-1;direct;".to_vec();
    input.extend(std::iter::repeat_n(b'a', 33 * 1024));
    input.push(0x07);
    assert!(scan(&input).is_empty());
}
