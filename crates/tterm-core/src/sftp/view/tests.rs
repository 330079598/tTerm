//! Specification tests transcribed from
//! `src/components/SftpDrawer/sftpView.test.ts` and
//! `src/components/SftpDrawer/sftpHistory.test.ts`.
//!
//! The view-preference storage read and write stay in the WebView; the tests
//! pass the stored text to the validator instead of a `Storage`.

use crate::sftp::entry::SftpDirectoryEntry;
use crate::sftp::view::format::{
    MAX_EDIT_FILE_BYTES, format_bytes, format_timestamp, format_timestamp_at,
};
use crate::sftp::view::history::{
    EMPTY_SFTP_HISTORY, SFTP_HISTORY_LIMIT, SftpHistory, drop_sftp_history_entry,
    land_sftp_history, record_sftp_visit, sftp_history_target,
};
use crate::sftp::view::paths::{is_permission_denied, join_remote_path, parent_remote_path};
use crate::sftp::view::sort::{
    DEFAULT_SFTP_VIEW_PREFERENCES, SftpSort, SftpSortColumn, SftpSortDirection, apply_sftp_view,
    format_octal_mode, next_sftp_sort, normalize_sftp_view_preferences, parse_octal_mode,
    sort_sftp_entries,
};

fn entry(name: &str) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        name: name.to_owned(),
        path: format!("/srv/{name}"),
        is_dir: false,
        is_symlink: false,
        size: None,
        modified_at: None,
        permissions: None,
        mode: None,
        owner: None,
        group: None,
    }
}

fn directory(name: &str, modified_at: i64) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        is_dir: true,
        modified_at: Some(modified_at),
        ..entry(name)
    }
}

fn file(name: &str, size: u64, modified_at: i64) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        size: Some(size),
        modified_at: Some(modified_at),
        ..entry(name)
    }
}

fn entries() -> Vec<SftpDirectoryEntry> {
    vec![
        file("notes.txt", 300, 3),
        directory("logs", 9),
        file("file10.log", 20, 1),
        file(".env", 5, 2),
        file("File2.log", 100, 4),
        directory("app", 5),
    ]
}

fn names(list: &[SftpDirectoryEntry]) -> Vec<String> {
    list.iter().map(|item| item.name.clone()).collect()
}

fn sort(column: SftpSortColumn, direction: SftpSortDirection) -> SftpSort {
    SftpSort { column, direction }
}

#[test]
fn sorts_names_naturally_and_case_insensitively_with_folders_first() {
    assert_eq!(
        names(&sort_sftp_entries(
            &entries(),
            sort(SftpSortColumn::Name, SftpSortDirection::Asc)
        )),
        vec![
            "app",
            "logs",
            ".env",
            "File2.log",
            "file10.log",
            "notes.txt"
        ]
    );
}

#[test]
fn keeps_folders_first_when_descending() {
    assert_eq!(
        names(&sort_sftp_entries(
            &entries(),
            sort(SftpSortColumn::Name, SftpSortDirection::Desc)
        )),
        vec![
            "logs",
            "app",
            "notes.txt",
            "file10.log",
            "File2.log",
            ".env"
        ]
    );
}

#[test]
fn sorts_by_size_and_by_modification_time() {
    let by_size = names(&sort_sftp_entries(
        &entries(),
        sort(SftpSortColumn::Size, SftpSortDirection::Desc),
    ));
    assert_eq!(
        by_size[2..],
        ["notes.txt", "File2.log", "file10.log", ".env"]
    );
    assert_eq!(
        names(&sort_sftp_entries(
            &entries(),
            sort(SftpSortColumn::Modified, SftpSortDirection::Asc)
        )),
        vec![
            "app",
            "logs",
            "file10.log",
            ".env",
            "notes.txt",
            "File2.log"
        ]
    );
}

#[test]
fn groups_files_by_extension_for_the_kind_column_with_names_breaking_ties() {
    let by_kind = names(&sort_sftp_entries(
        &entries(),
        sort(SftpSortColumn::Kind, SftpSortDirection::Asc),
    ));
    assert_eq!(
        by_kind[2..],
        [".env", "File2.log", "file10.log", "notes.txt"]
    );
}

#[test]
fn does_not_mutate_the_listing() {
    let before = names(&entries());
    sort_sftp_entries(
        &entries(),
        sort(SftpSortColumn::Size, SftpSortDirection::Asc),
    );
    assert_eq!(names(&entries()), before);
}

#[test]
fn drops_dotfiles_when_hidden_files_are_off() {
    assert_eq!(
        names(&apply_sftp_view(
            &entries(),
            crate::sftp::view::sort::SftpViewPreferences {
                show_hidden: false,
                ..DEFAULT_SFTP_VIEW_PREFERENCES
            }
        )),
        vec!["app", "logs", "File2.log", "file10.log", "notes.txt"]
    );
}

#[test]
fn flips_the_direction_of_the_sorted_column_and_resets_for_another() {
    assert_eq!(
        next_sftp_sort(
            sort(SftpSortColumn::Name, SftpSortDirection::Asc),
            SftpSortColumn::Name
        ),
        sort(SftpSortColumn::Name, SftpSortDirection::Desc)
    );
    assert_eq!(
        next_sftp_sort(
            sort(SftpSortColumn::Name, SftpSortDirection::Desc),
            SftpSortColumn::Size
        ),
        sort(SftpSortColumn::Size, SftpSortDirection::Asc)
    );
}

#[test]
fn round_trips_view_preferences_and_ignores_malformed_values() {
    assert_eq!(
        normalize_sftp_view_preferences(None),
        DEFAULT_SFTP_VIEW_PREFERENCES
    );

    let preferences = crate::sftp::view::sort::SftpViewPreferences {
        sort: sort(SftpSortColumn::Size, SftpSortDirection::Desc),
        show_hidden: false,
    };
    let stored = r#"{"sort":{"column":"size","direction":"desc"},"showHidden":false}"#;
    assert_eq!(normalize_sftp_view_preferences(Some(stored)), preferences);

    assert_eq!(
        normalize_sftp_view_preferences(Some(
            r#"{"sort":{"column":"bogus","direction":"up"},"showHidden":1}"#
        )),
        DEFAULT_SFTP_VIEW_PREFERENCES
    );
    assert_eq!(
        normalize_sftp_view_preferences(Some("not json")),
        DEFAULT_SFTP_VIEW_PREFERENCES
    );
}

#[test]
fn parses_three_or_four_octal_digits_only() {
    assert_eq!(parse_octal_mode("644"), Some(0o644));
    assert_eq!(parse_octal_mode(" 2755 "), Some(0o2755));
    assert_eq!(parse_octal_mode("64"), None);
    assert_eq!(parse_octal_mode("688"), None);
    assert_eq!(parse_octal_mode("07777 "), None);
    assert_eq!(parse_octal_mode("rwx"), None);
}

#[test]
fn formats_modes_with_at_least_three_digits() {
    assert_eq!(format_octal_mode(0o644), "644");
    assert_eq!(format_octal_mode(0o7), "007");
    assert_eq!(format_octal_mode(0o4755), "4755");
    assert_eq!(format_octal_mode(u32::MAX), "7777");
}

fn visit(paths: &[&str]) -> SftpHistory {
    paths.iter().fold(EMPTY_SFTP_HISTORY, |history, path| {
        record_sftp_visit(&history, path)
    })
}

#[test]
fn appends_new_directories_and_points_at_the_latest() {
    assert_eq!(
        visit(&["/home", "/var", "/var/log"]),
        SftpHistory {
            paths: vec!["/home".to_owned(), "/var".to_owned(), "/var/log".to_owned()],
            index: 2,
        }
    );
}

#[test]
fn ignores_reloads_of_the_current_directory() {
    let history = visit(&["/home", "/var"]);
    assert_eq!(record_sftp_visit(&history, "/var"), history);
}

#[test]
fn drops_forward_entries_when_navigating_somewhere_new() {
    let history = land_sftp_history(&visit(&["/home", "/var", "/var/log"]), 0, "/home");
    assert_eq!(
        record_sftp_visit(&history, "/etc"),
        SftpHistory {
            paths: vec!["/home".to_owned(), "/etc".to_owned()],
            index: 1,
        }
    );
}

#[test]
fn keeps_only_the_most_recent_entries() {
    let paths: Vec<String> = (0..SFTP_HISTORY_LIMIT + 5)
        .map(|index| format!("/d{index}"))
        .collect();
    let history = paths.iter().fold(EMPTY_SFTP_HISTORY, |history, path| {
        record_sftp_visit(&history, path)
    });
    assert_eq!(history.paths.len(), SFTP_HISTORY_LIMIT);
    assert_eq!(history.paths[0], "/d5");
    assert_eq!(history.index, SFTP_HISTORY_LIMIT as i32 - 1);
}

#[test]
fn returns_none_at_either_end_of_the_history() {
    use crate::sftp::view::history::SftpHistoryStep;

    assert_eq!(
        sftp_history_target(&EMPTY_SFTP_HISTORY, SftpHistoryStep::Back),
        None
    );
    let history = visit(&["/home", "/var"]);
    assert_eq!(
        sftp_history_target(&history, SftpHistoryStep::Forward),
        None
    );
    assert_eq!(
        sftp_history_target(&history, SftpHistoryStep::Back),
        Some(0)
    );
    assert_eq!(
        sftp_history_target(
            &land_sftp_history(&history, 0, "/home"),
            SftpHistoryStep::Forward
        ),
        Some(1)
    );
}

#[test]
fn keeps_forward_entries_when_stepping_back() {
    assert_eq!(
        land_sftp_history(&visit(&["/home", "/var"]), 0, "/home"),
        SftpHistory {
            paths: vec!["/home".to_owned(), "/var".to_owned()],
            index: 0,
        }
    );
}

#[test]
fn stores_the_path_the_server_resolved() {
    assert_eq!(
        land_sftp_history(&visit(&["/link", "/var"]), 0, "/real"),
        SftpHistory {
            paths: vec!["/real".to_owned(), "/var".to_owned()],
            index: 0,
        }
    );
}

#[test]
fn falls_back_to_a_regular_visit_for_an_index_that_no_longer_exists() {
    assert_eq!(
        land_sftp_history(&visit(&["/home"]), 3, "/var"),
        SftpHistory {
            paths: vec!["/home".to_owned(), "/var".to_owned()],
            index: 1,
        }
    );
}

#[test]
fn removes_an_entry_behind_the_current_one() {
    assert_eq!(
        drop_sftp_history_entry(&visit(&["/a", "/b", "/c"]), 1),
        SftpHistory {
            paths: vec!["/a".to_owned(), "/c".to_owned()],
            index: 1,
        }
    );
}

#[test]
fn removes_an_entry_ahead_of_the_current_one() {
    let history = land_sftp_history(&visit(&["/a", "/b", "/c"]), 0, "/a");
    assert_eq!(
        drop_sftp_history_entry(&history, 1),
        SftpHistory {
            paths: vec!["/a".to_owned(), "/c".to_owned()],
            index: 0,
        }
    );
}

#[test]
fn never_removes_the_directory_on_screen() {
    let history = visit(&["/a", "/b"]);
    assert_eq!(drop_sftp_history_entry(&history, 1), history);
}

#[test]
fn joins_and_goes_up_from_remote_paths_without_normalizing_the_text() {
    assert_eq!(join_remote_path("/", "a"), "/a");
    assert_eq!(join_remote_path("/srv", "a"), "/srv/a");
    assert_eq!(join_remote_path("/srv/", "a"), "/srv//a");

    assert_eq!(parent_remote_path("/a/b"), "/a");
    assert_eq!(parent_remote_path("/a"), "/");
    assert_eq!(parent_remote_path("/"), "/");
    assert_eq!(parent_remote_path(""), "/");
}

#[test]
fn recognizes_permission_denied_errors_without_case() {
    assert!(is_permission_denied(Some("Permission denied")));
    assert!(is_permission_denied(Some(
        "permission denied (SSH_FX_PERMISSION_DENIED)"
    )));
    assert!(!is_permission_denied(None));
}

#[test]
fn formats_sizes_with_the_drawers_units_and_precision() {
    assert_eq!(format_bytes(None), "--");
    assert_eq!(format_bytes(Some(0)), "0 B");
    assert_eq!(format_bytes(Some(1023)), "1023 B");
    assert_eq!(format_bytes(Some(1024)), "1.0 KB");
    assert_eq!(format_bytes(Some(1280)), "1.3 KB");
    assert_eq!(format_bytes(Some(1536)), "1.5 KB");
    assert_eq!(format_bytes(Some(10_240)), "10 KB");
    assert_eq!(format_bytes(Some(1024 * 1024 - 1)), "1024 KB");
    assert_eq!(format_bytes(Some(5 * 1024 * 1024)), "5.0 MB");
    assert_eq!(format_bytes(Some(1024_u64.pow(4))), "1.0 TB");
    assert_eq!(MAX_EDIT_FILE_BYTES, 5_242_880);
}

#[test]
fn formats_timestamps_in_local_time_and_leaves_zero_empty() {
    use chrono::{Local, TimeZone};

    assert_eq!(format_timestamp(None), "--");
    assert_eq!(format_timestamp(Some(0)), "--");
    let local = Local
        .with_ymd_and_hms(2026, 1, 1, 9, 5, 7)
        .single()
        .expect("a local test time");
    assert_eq!(format_timestamp_at(local), "2026-01-01 09:05:07");
}
