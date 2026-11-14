use serde_json::Value;

use super::{
    DEFAULT_SCROLLBACK_LINES, MAX_EXPLICIT_SCROLLBACK_LINES, UNLIMITED_SCROLLBACK_BUFFER,
    UNLIMITED_SCROLLBACK_INITIAL, UNLIMITED_SCROLLBACK_SENTINEL, count_line_feeds,
    grown_scrollback_capacity, is_unlimited_scrollback, normalize_scrollback_config,
    resolve_scrollback_lines, starting_scrollback_lines,
};

/// The expectations generated from the real TypeScript module by
/// `scripts/migration/P1-11c-oracle.mjs`.
fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

#[test]
fn starts_unlimited_scrollback_small_instead_of_at_the_full_buffer() {
    assert_eq!(
        starting_scrollback_lines(Some(0.0)),
        UNLIMITED_SCROLLBACK_INITIAL
    );
}

#[test]
fn passes_explicit_limits_through() {
    assert_eq!(starting_scrollback_lines(Some(5000.0)), 5000);
    assert_eq!(starting_scrollback_lines(None), 10_000);
}

#[test]
fn counts_lf_vt_and_ff_but_not_cr() {
    assert_eq!(count_line_feeds("a\r\nb\nc\u{0b}d\u{0c}e\r"), 4);
    assert_eq!(count_line_feeds(""), 0);
    assert_eq!(count_line_feeds("Japan\n\r"), 1);
}

#[test]
fn keeps_the_capacity_while_half_of_it_is_still_free() {
    assert_eq!(grown_scrollback_capacity(10_000, 5000), None);
}

#[test]
fn doubles_until_the_lines_fit_twice_over() {
    assert_eq!(grown_scrollback_capacity(10_000, 5001), Some(20_000));
    assert_eq!(grown_scrollback_capacity(10_000, 30_000), Some(80_000));
}

#[test]
fn never_grows_past_the_unlimited_buffer() {
    assert_eq!(
        grown_scrollback_capacity(10_000, UNLIMITED_SCROLLBACK_BUFFER),
        Some(UNLIMITED_SCROLLBACK_BUFFER)
    );
}

#[test]
fn normalizes_rounded_values_with_the_sentinel_and_limits() {
    assert_eq!(normalize_scrollback_config(None), DEFAULT_SCROLLBACK_LINES);
    assert_eq!(
        normalize_scrollback_config(Some(f64::NAN)),
        DEFAULT_SCROLLBACK_LINES
    );
    assert_eq!(
        normalize_scrollback_config(Some(f64::INFINITY)),
        DEFAULT_SCROLLBACK_LINES
    );
    assert_eq!(
        normalize_scrollback_config(Some(0.0)),
        UNLIMITED_SCROLLBACK_SENTINEL
    );
    assert_eq!(normalize_scrollback_config(Some(1.0)), 1);
    assert_eq!(normalize_scrollback_config(Some(0.4)), 0);
    assert_eq!(normalize_scrollback_config(Some(0.6)), 1);
    assert_eq!(normalize_scrollback_config(Some(-3.0)), 1);
    assert_eq!(
        normalize_scrollback_config(Some(MAX_EXPLICIT_SCROLLBACK_LINES as f64)),
        MAX_EXPLICIT_SCROLLBACK_LINES
    );
    assert_eq!(
        normalize_scrollback_config(Some(MAX_EXPLICIT_SCROLLBACK_LINES as f64 + 1.0)),
        MAX_EXPLICIT_SCROLLBACK_LINES
    );
}

#[test]
fn resolves_the_sentinel_to_the_full_buffer() {
    assert_eq!(
        resolve_scrollback_lines(Some(0.0)),
        UNLIMITED_SCROLLBACK_BUFFER
    );
    assert_eq!(resolve_scrollback_lines(Some(5000.0)), 5000);
    assert_eq!(resolve_scrollback_lines(None), DEFAULT_SCROLLBACK_LINES);
}

#[test]
fn treats_only_the_sentinel_as_unlimited() {
    assert!(is_unlimited_scrollback(Some(0.0)));
    assert!(!is_unlimited_scrollback(Some(1.0)));
    assert!(!is_unlimited_scrollback(None));
}

#[test]
fn grows_from_the_initial_room_and_stops_at_the_buffer() {
    assert_eq!(grown_scrollback_capacity(0, 1), Some(10_000));
    assert_eq!(grown_scrollback_capacity(0, 5000), Some(10_000));
    assert_eq!(
        grown_scrollback_capacity(0, u32::MAX),
        Some(UNLIMITED_SCROLLBACK_BUFFER)
    );
    assert_eq!(grown_scrollback_capacity(u32::MAX, 0), None);
}

#[test]
fn oracle_matches_the_ported_scrollback_helpers() {
    let oracle = oracle();
    let values = &oracle["values"];

    let constants = &values["scrollback.constants"];
    assert_eq!(
        constants["DEFAULT_SCROLLBACK_LINES"].as_u64(),
        Some(u64::from(DEFAULT_SCROLLBACK_LINES))
    );
    assert_eq!(
        constants["UNLIMITED_SCROLLBACK_SENTINEL"].as_u64(),
        Some(u64::from(UNLIMITED_SCROLLBACK_SENTINEL))
    );
    assert_eq!(
        constants["UNLIMITED_SCROLLBACK_BUFFER"].as_u64(),
        Some(u64::from(UNLIMITED_SCROLLBACK_BUFFER))
    );
    assert_eq!(
        constants["MAX_EXPLICIT_SCROLLBACK_LINES"].as_u64(),
        Some(u64::from(MAX_EXPLICIT_SCROLLBACK_LINES))
    );
    assert_eq!(
        constants["UNLIMITED_SCROLLBACK_INITIAL"].as_u64(),
        Some(u64::from(UNLIMITED_SCROLLBACK_INITIAL))
    );

    let normalize = &values["scrollback.normalize"];
    let inputs = normalize["inputs"].as_array().expect("inputs");
    let stored = normalize["stored"].as_array().expect("stored");
    for (input, expected) in inputs.iter().zip(stored) {
        let input = input.as_f64();
        assert_eq!(
            normalize_scrollback_config(input),
            expected.as_u64().expect("stored value") as u32,
            "normalize {input:?}"
        );
    }

    let resolved = normalize["resolved"].as_array().expect("resolved");
    assert_eq!(
        resolve_scrollback_lines(None),
        resolved[0].as_u64().unwrap() as u32
    );
    assert_eq!(
        resolve_scrollback_lines(Some(0.0)),
        resolved[1].as_u64().unwrap() as u32
    );

    let starting = normalize["starting"].as_array().expect("starting");
    assert_eq!(
        starting_scrollback_lines(None),
        starting[0].as_u64().unwrap() as u32
    );
    assert_eq!(
        starting_scrollback_lines(Some(0.0)),
        starting[1].as_u64().unwrap() as u32
    );
    assert_eq!(
        starting_scrollback_lines(Some(5000.0)),
        starting[2].as_u64().unwrap() as u32
    );

    let unlimited = normalize["unlimited"].as_array().expect("unlimited");
    assert_eq!(
        is_unlimited_scrollback(None),
        unlimited[0].as_bool().unwrap()
    );
    assert_eq!(
        is_unlimited_scrollback(Some(0.0)),
        unlimited[1].as_bool().unwrap()
    );
    assert_eq!(
        is_unlimited_scrollback(Some(1.0)),
        unlimited[2].as_bool().unwrap()
    );
}
