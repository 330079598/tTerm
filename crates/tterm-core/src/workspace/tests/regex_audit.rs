use regress::{Flags, Regex};

use super::oracle_11d2;

#[test]
fn oracle_regex_audit_matches_the_engine_used_by_the_port() {
    let oracle = oracle_11d2();
    let cases = oracle["regexAudit"].as_array().expect("regex cases");
    assert!(!cases.is_empty());
    for case in cases {
        let pattern = case["pattern"].as_str().expect("pattern");
        let flags = case["flags"].as_str().expect("flags");
        let regex = Regex::with_flags(
            pattern,
            Flags {
                icase: flags.contains('i'),
                ..Flags::default()
            },
        )
        .unwrap_or_else(|error| panic!("{pattern:?} {flags:?}: {error}"));
        for (kind, expected) in [("positive", true), ("negative", false)] {
            let input = case[kind]["input"].as_str().expect("input");
            assert_eq!(
                regex.find(input).is_some(),
                expected,
                "{pattern:?} {kind} {input:?}"
            );
            assert_eq!(
                case[kind]["matches"].as_bool().expect("node result"),
                expected,
                "{pattern:?} {kind} node disagrees"
            );
        }
    }
}
