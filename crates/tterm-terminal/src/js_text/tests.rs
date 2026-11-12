use super::{
    from_utf16_units_lossy, is_control, js_trim, js_trim_end, utf16_insert_at, utf16_len,
    utf16_remove_at, utf16_remove_before, utf16_slice, utf16_slice_from, utf16_tail, utf16_units,
};

#[test]
fn lengths_count_utf16_code_units() {
    assert_eq!(utf16_len(""), 0);
    assert_eq!(utf16_len("abc"), 3);
    assert_eq!(utf16_len("a😀中文"), 5);
    assert_eq!(utf16_len("e\u{0301}"), 2);
}

#[test]
fn units_are_utf16_code_units() {
    assert_eq!(
        utf16_units("a😀中文"),
        vec![0x61, 0xd83d, 0xde00, 0x4e2d, 0x6587]
    );
}

#[test]
fn slices_follow_utf16_positions() {
    assert_eq!(utf16_slice("a😀bc", 0, 3), "a😀");
    assert_eq!(utf16_slice("a😀bc", 1, 3), "😀");
    assert_eq!(utf16_slice_from("a😀bc", 3), "bc");
    assert_eq!(utf16_slice("abc", 5, 2), "");
}

#[test]
fn a_slice_mid_surrogate_becomes_a_replacement() {
    // JavaScript returns "a\uD83D" here; a Rust string cannot hold the lone
    // surrogate, so the lossy conversion is the closest representation.
    assert_eq!(utf16_slice("a😀bc", 0, 2), "a\u{FFFD}");
    assert_eq!(from_utf16_units_lossy(&[0x61, 0xd83d]), "a\u{FFFD}");
}

#[test]
fn tails_keep_whole_characters_when_the_boundary_allows() {
    assert_eq!(utf16_tail("abc", 0), "abc");
    assert_eq!(utf16_tail("abc", 2), "bc");
    assert_eq!(utf16_tail("a😀", 2), "😀");
    assert_eq!(utf16_tail("a😀bc", 4), "😀bc");
}

#[test]
fn insertion_and_removal_use_utf16_positions() {
    assert_eq!(utf16_insert_at("😀a", 2, "中"), "😀中a");
    assert_eq!(utf16_remove_at("a😀b", 3), "a😀");
    assert_eq!(utf16_remove_before("abc", 2), "ac");
}

#[test]
fn trims_javascript_whitespace() {
    assert_eq!(js_trim("  hello  "), "hello");
    assert_eq!(js_trim("\u{3000}中文\u{00a0}"), "中文");
    assert_eq!(js_trim("\u{feff}x\u{feff}"), "x");
    assert_eq!(js_trim_end("x\u{2028}"), "x");
    assert_eq!(js_trim("x\u{2028}y"), "x\u{2028}y");
}

#[test]
fn control_is_the_cc_category() {
    assert!(is_control('\u{0000}'));
    assert!(is_control('\u{001f}'));
    assert!(is_control('\u{007f}'));
    assert!(is_control('\u{0085}'));
    assert!(is_control('\u{009f}'));
    assert!(!is_control('\u{00a0}'));
    assert!(!is_control('\u{2028}'));
}
