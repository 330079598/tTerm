use super::{
    TerminalOutputScanState, connecting_carry_chars, connecting_marker_len, decode_output_chunk,
    empty_output_scan_state, scan_terminal_output,
};
use crate::js_text::utf16_len;

#[test]
fn carries_one_less_unit_than_the_connecting_marker() {
    assert_eq!(connecting_marker_len(), 11);
    assert_eq!(connecting_carry_chars(), 10);
}

#[test]
fn keeps_only_a_marker_sized_carry_under_a_flood() {
    let mut state = empty_output_scan_state();
    for _ in 0..50 {
        state = scan_terminal_output(&state, &"x".repeat(1000)).state;
    }
    assert!(utf16_len(&state.tail) < "[Connecting".len());
}

#[test]
fn detects_cold_path_connecting_status_lines() {
    let result = scan_terminal_output(
        &empty_output_scan_state(),
        "\r\n\x1b[33m[Connecting to jump host #1 user@host:22]\x1b[0m\r\n",
    );
    assert!(result.connecting);
}

#[test]
fn does_not_flag_a_connecting_marker_left_over_in_the_rolling_tail() {
    let first = scan_terminal_output(
        &empty_output_scan_state(),
        "\r\n\x1b[33m[Connecting to jump host #1 user@host:22]\x1b[0m\r\n",
    );
    let second = scan_terminal_output(&first.state, "$ grep Connect logs\n");
    assert!(!second.connecting);
}

#[test]
fn detects_a_connecting_marker_split_across_chunk_boundaries() {
    let first = scan_terminal_output(&empty_output_scan_state(), "\r\n\x1b[33m[Conn");
    assert!(!first.connecting);
    let second = scan_terminal_output(&first.state, "ecting to user@host:22]\x1b[0m\r\n");
    assert!(second.connecting);
}

#[test]
fn reports_connecting_false_for_ordinary_output() {
    let result = scan_terminal_output(&empty_output_scan_state(), "$ echo hi\nhi\n");
    assert!(!result.connecting);
}

#[test]
fn keeps_a_ten_unit_tail_across_emoji_chunks() {
    let first = scan_terminal_output(&empty_output_scan_state(), &"😀".repeat(8));
    assert_eq!(first.state.tail, "😀".repeat(5));
    let second = scan_terminal_output(&first.state, "x");
    assert_eq!(utf16_len(&second.state.tail), 10);
    // Node's slice starts on the low surrogate here and returns a lone
    // surrogate; a Rust String cannot represent it, so the lossy conversion
    // records a replacement instead.
    assert_eq!(second.state.tail, format!("\u{FFFD}{}x", "😀".repeat(4)));
}

#[test]
fn decodes_valid_utf8_bytes() {
    assert_eq!(decode_output_chunk("héllo 世界".as_bytes()), "héllo 世界");
}

#[test]
fn falls_back_to_replacement_characters_for_malformed_bytes() {
    assert_eq!(decode_output_chunk(&[0xe4, 0xb8]), "\u{FFFD}");
}

#[test]
fn keeps_valid_bytes_around_a_malformed_sequence() {
    assert_eq!(decode_output_chunk(&[0x61, 0xe4, 0xb8, 0x62]), "a\u{FFFD}b");
}

#[test]
fn decodes_control_characters_and_escape_sequences() {
    let payload = "\r\n\x1b[31mred\x1b[0m";
    assert_eq!(decode_output_chunk(payload.as_bytes()), payload);
}

#[test]
fn drops_one_leading_byte_order_mark_like_text_decoder() {
    assert_eq!(decode_output_chunk(&[0xEF, 0xBB, 0xBF, 0x61]), "a");
    assert_eq!(decode_output_chunk(&[0xEF, 0xBB, 0xBF]), "");
    assert_eq!(
        decode_output_chunk(&[0xEF, 0xBB, 0xBF, 0xEF, 0xBB, 0xBF]),
        "\u{FEFF}"
    );
}

#[test]
fn malformed_utf8_matches_text_decoder_replacements() {
    assert_eq!(decode_output_chunk(&[0xf0, 0x9f]), "\u{FFFD}");
    assert_eq!(decode_output_chunk(&[0xf0, 0x9f, 0x92]), "\u{FFFD}");
    assert_eq!(
        decode_output_chunk(&[0xe0, 0x80, 0x80]),
        "\u{FFFD}\u{FFFD}\u{FFFD}"
    );
    assert_eq!(
        decode_output_chunk(&[0xed, 0xa0, 0x80]),
        "\u{FFFD}\u{FFFD}\u{FFFD}"
    );
    assert_eq!(
        decode_output_chunk(&[0xf4, 0x90, 0x80, 0x80]),
        "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"
    );
    assert_eq!(decode_output_chunk(&[0xc0, 0xaf]), "\u{FFFD}\u{FFFD}");
}

#[test]
fn empty_state_starts_with_no_tail() {
    assert_eq!(
        empty_output_scan_state(),
        TerminalOutputScanState::default()
    );
}
