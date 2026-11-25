//! Pure validation tests for the cursor and padding drafts.

use super::*;

#[test]
fn padding_accepts_the_saved_range_and_trims_spaces() {
    assert_eq!(parse_padding("0"), Ok(0));
    assert_eq!(parse_padding(" 12 "), Ok(12));
    assert_eq!(parse_padding("80"), Ok(80));
}

#[test]
fn padding_refuses_empty_fractional_negative_and_large_values() {
    assert_eq!(parse_padding(""), Err(PaddingError::Required));
    assert_eq!(parse_padding("  "), Err(PaddingError::Required));
    assert_eq!(parse_padding("1.5"), Err(PaddingError::Invalid));
    assert_eq!(parse_padding("-1"), Err(PaddingError::Invalid));
    assert_eq!(parse_padding("eighty"), Err(PaddingError::Invalid));
    assert_eq!(parse_padding("81"), Err(PaddingError::Range));
}

#[test]
fn cursor_style_keeps_a_known_shape_and_falls_back_to_block() {
    assert_eq!(cursor_style("bar"), "bar");
    assert_eq!(cursor_style("underline"), "underline");
    assert_eq!(cursor_style("block"), "block");
    assert_eq!(cursor_style("beam"), "block");
}
