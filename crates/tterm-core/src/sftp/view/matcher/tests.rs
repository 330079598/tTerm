//! Specification tests from `src/components/SftpDrawer/sftpSearch.ts` and
//! `ts-oracle.txt`.

use crate::sftp::entry::SftpDirectoryEntry;
use crate::sftp::internal::api::search::matcher::NameMatcher;
use crate::sftp::view::matcher::{MATCH_ALL, SftpSearchOptions, create_sftp_search_matcher};

const NAMES: [&str; 6] = [
    "README.md",
    "readme.txt",
    "a.log",
    "A.LOG",
    "b1",
    "档案.txt",
];

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

fn hits(query: &str, options: SftpSearchOptions) -> Vec<&'static str> {
    let matcher = create_sftp_search_matcher(query, options);
    NAMES
        .into_iter()
        .filter(|name| matcher.matches_name(name))
        .collect()
}

#[test]
fn plain_text_matches_anywhere_without_case() {
    assert_eq!(
        hits("read", SftpSearchOptions::default()),
        ["README.md", "readme.txt"]
    );
}

#[test]
fn globs_match_the_whole_name() {
    assert_eq!(
        hits("*.log", SftpSearchOptions::default()),
        ["a.log", "A.LOG"]
    );
}

#[test]
fn glob_classes_support_negation() {
    assert_eq!(
        hits("[!a]*", SftpSearchOptions::default()),
        ["README.md", "readme.txt", "b1", "档案.txt"]
    );
}

#[test]
fn regex_option_takes_a_javascript_regular_expression() {
    let regex = SftpSearchOptions {
        regex: true,
        ..SftpSearchOptions::default()
    };
    assert_eq!(hits(r"^a\.log$", regex), ["a.log", "A.LOG"]);

    // Without the `u` flag, as in the WebView, case folding does not turn
    // the Kelvin sign into `k`.
    assert!(!create_sftp_search_matcher("k", regex).matches_name("K"));
}

#[test]
fn invalid_regex_falls_back_to_glob_or_reports_an_error() {
    let regex = SftpSearchOptions {
        regex: true,
        ..SftpSearchOptions::default()
    };
    let fallback = create_sftp_search_matcher("a[", regex);
    assert!(fallback.error.is_none());
    assert!(fallback.has_query);
    assert!(hits("a[", regex).is_empty());

    let invalid = create_sftp_search_matcher("a(", regex);
    assert!(invalid.has_query);
    assert!(
        invalid
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("Invalid regular expression: /a(/i:"))
    );
    // `regress` calls V8's unterminated group an unbalanced parenthesis.
    assert!(
        invalid
            .error
            .as_deref()
            .is_some_and(|error| error.contains("Unbalanced parenthesis"))
    );
    assert!(hits("a(", regex).is_empty());
}

#[test]
fn empty_queries_and_match_all_let_every_name_through() {
    let empty = create_sftp_search_matcher("   ", SftpSearchOptions::default());
    assert!(!empty.has_query);
    assert!(empty.error.is_none());
    assert_eq!(hits("   ", SftpSearchOptions::default()), NAMES);
    assert!(MATCH_ALL.matches(&entry("x")));
}

#[test]
fn plain_and_glob_queries_agree_with_the_subfolder_search() {
    for query in ["read", "*.log", "?.log", "[!a]*"]
    // The subfolder search still uses the `regex` crate and its own class
    // escaping; these are the shapes both implementations promise to share.
    {
        let backend = NameMatcher::new(query, false).expect("a valid backend matcher");
        let drawer = create_sftp_search_matcher(query, SftpSearchOptions::default());
        let backend_hits: Vec<&str> = NAMES
            .into_iter()
            .filter(|name| backend.matches(name))
            .collect();
        assert_eq!(hits(query, SftpSearchOptions::default()), backend_hits);
        assert!(drawer.error.is_none());
    }
}
