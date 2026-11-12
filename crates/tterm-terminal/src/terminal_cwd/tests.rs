use super::{
    CWD_REPORT_OSC_CODES, Osc7Report, is_same_host, is_unnamed_host, parse_cwd_report,
    parse_osc7_report,
};

#[test]
fn carries_the_osc_codes_the_frontend_uses() {
    assert_eq!(CWD_REPORT_OSC_CODES, [7, 9]);
}

#[test]
fn reads_osc7_paths_sent_by_tterm_posix_shell_integration() {
    assert_eq!(
        parse_cwd_report(7, "file:///home/me/project").as_deref(),
        Some("/home/me/project")
    );
    assert_eq!(
        parse_cwd_report(7, "file:///c/Users/me").as_deref(),
        Some("/c/Users/me")
    );
    assert_eq!(
        parse_cwd_report(7, "file://localhost/tmp").as_deref(),
        Some("/tmp")
    );
}

#[test]
fn decodes_escapes_and_keeps_raw_characters() {
    assert_eq!(
        parse_cwd_report(7, "file:///home/me/a b%2520中").as_deref(),
        Some("/home/me/a b%20中")
    );
    assert_eq!(
        parse_cwd_report(7, "file:///home/me/caf%C3%A9").as_deref(),
        Some("/home/me/café")
    );
    assert_eq!(
        parse_cwd_report(7, "file:///home/me/100%").as_deref(),
        Some("/home/me/100%")
    );
    assert_eq!(
        parse_cwd_report(7, "file:///tmp/%F0%9F%98%80").as_deref(),
        Some("/tmp/😀")
    );
}

#[test]
fn turns_a_file_url_with_a_drive_letter_into_a_windows_path() {
    assert_eq!(
        parse_cwd_report(7, "file:///C:/Users/me").as_deref(),
        Some("C:/Users/me")
    );
    assert_eq!(parse_cwd_report(7, "file:///D:").as_deref(), Some("D:"));
}

#[test]
fn ignores_osc7_from_other_hosts_and_malformed_payloads() {
    assert_eq!(parse_cwd_report(7, "file://build-server/home/me"), None);
    assert_eq!(parse_cwd_report(7, "/home/me"), None);
    assert_eq!(parse_cwd_report(7, "file://"), None);
}

#[test]
fn an_osc7_path_does_not_end_before_a_trailing_newline() {
    assert_eq!(parse_cwd_report(7, "file:///tmp\n"), None);
    assert_eq!(parse_osc7_report("file:///tmp\n"), None);
}

#[test]
fn reads_osc9_paths_from_cmd_and_powershell() {
    assert_eq!(
        parse_cwd_report(9, "9;\"C:\\Users\\me\\My Projects\"").as_deref(),
        Some("C:\\Users\\me\\My Projects")
    );
    assert_eq!(
        parse_cwd_report(9, "9;C:\\Windows").as_deref(),
        Some("C:\\Windows")
    );
    assert_eq!(
        parse_cwd_report(9, "9;\\\\server\\share\\dir").as_deref(),
        Some("\\\\server\\share\\dir")
    );
}

#[test]
fn ignores_other_osc9_messages() {
    assert_eq!(parse_cwd_report(9, "4;1;50"), None);
    assert_eq!(parse_cwd_report(9, "Build finished"), None);
    assert_eq!(parse_cwd_report(9, "9;\"\""), None);
    assert_eq!(parse_cwd_report(9, "9;\n"), None);
    assert_eq!(parse_cwd_report(133, "A"), None);
}

#[test]
fn keeps_the_host_in_lower_case_beside_the_path() {
    assert_eq!(
        parse_osc7_report("file://Web01/srv/my%20app"),
        Some(Osc7Report {
            host: "web01".to_owned(),
            path: "/srv/my app".to_owned(),
        })
    );
    assert_eq!(
        parse_osc7_report("file:///tmp"),
        Some(Osc7Report {
            host: String::new(),
            path: "/tmp".to_owned(),
        })
    );
    assert_eq!(parse_osc7_report("file://web01"), None);
    assert_eq!(
        parse_cwd_report(7, "file://Web01/srv"),
        None,
        "a named host is not the local shell"
    );
}

#[test]
fn treats_a_missing_host_and_localhost_as_no_name() {
    assert!(is_unnamed_host(""));
    assert!(is_unnamed_host("LocalHost"));
    assert!(!is_unnamed_host("web01"));
}

#[test]
fn compares_only_the_first_label() {
    assert!(is_same_host("web01", "WEB01.example.com"));
    assert!(is_same_host("web01.a.com", "web01.b.com"));
    assert!(!is_same_host("web01", "web02"));
}
